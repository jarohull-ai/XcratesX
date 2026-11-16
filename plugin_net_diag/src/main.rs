use std::io::{self, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use shared_contract::{JfpMessage, JfpReader};

#[derive(Default)]
struct State {
    last_report: Option<String>,
    check_count: u64,
}

fn check_tcp_port(addr_str: &str, timeout: Duration) -> (bool, u128, String) {
    let start = Instant::now();
    match addr_str.to_socket_addrs() {
        Ok(mut addrs) => {
            if let Some(addr) = addrs.next() {
                match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(_) => {
                        let elapsed = start.elapsed().as_micros();
                        (true, elapsed, format!("{addr} OPEN ({elapsed} us)"))
                    }
                    Err(e) => {
                        let elapsed = start.elapsed().as_micros();
                        (false, elapsed, format!("{addr} FAIL ({e})"))
                    }
                }
            } else {
                (false, 0, "No valid IP address".to_string())
            }
        }
        Err(e) => (false, 0, format!("Address resolution failed: {e}")),
    }
}

fn run_cluster_diagnostic() -> String {
    let mut out = String::new();
    out.push_str("=== TCP CONNECTIVITY CHECK ===\n");

    let configured = std::env::var("XCRATESX_TCP_TARGETS")
        .unwrap_or_else(|_| "127.0.0.1:22,127.0.0.1:443".into());
    let targets = configured
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(16);

    for addr in targets {
        let name = addr;
        let (ok, micros, desc) = check_tcp_port(addr, Duration::from_millis(500));
        let badge = if ok { "[OK]   " } else { "[FAIL] " };
        let latency_ms = micros as f64 / 1000.0;
        out.push_str(&format!(
            "{badge} {name:<32} -> {desc} (latency: {latency_ms:.2} ms)\n"
        ));
    }

    out.push_str("\nCommands: check | ping <ip>:<port> | port <ip> <port> | help");
    out
}

fn handle(frame: &JfpMessage, state: &mut State) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let input = frame.body.trim();
            let parts: Vec<&str> = input.split_whitespace().collect();

            if parts.is_empty() {
                return Some(JfpMessage::reply(
                    "OK",
                    "NET_DIAG | Enter a command: check, ping <host:port>, port <host> <port>, help",
                ));
            }

            match parts[0] {
                "check" | "diag" | "test" => {
                    state.check_count += 1;
                    let report = run_cluster_diagnostic();
                    state.last_report = Some(report.clone());
                    Some(JfpMessage::reply("OK", report))
                }
                "ping" => {
                    if parts.len() < 2 {
                        Some(JfpMessage::reply("ERROR", "Usage: ping <host:port>"))
                    } else {
                        let target = parts[1];
                        let addr = if target.contains(':') {
                            target.to_string()
                        } else {
                            format!("{target}:22")
                        };
                        let (ok, micros, desc) = check_tcp_port(&addr, Duration::from_millis(800));
                        let status = if ok { "OK" } else { "FAIL" };
                        let ms = micros as f64 / 1000.0;
                        let body = format!("NET_DIAG PING [{status}] {addr} -> {desc} | time: {ms:.3} ms");
                        state.last_report = Some(body.clone());
                        Some(JfpMessage::reply(status, body))
                    }
                }
                "port" => {
                    if parts.len() < 3 {
                        Some(JfpMessage::reply("ERROR", "Usage: port <host> <port>"))
                    } else {
                        let host = parts[1];
                        let port = parts[2];
                        let target = format!("{host}:{port}");
                        let (ok, micros, desc) = check_tcp_port(&target, Duration::from_millis(800));
                        let status = if ok { "OK" } else { "FAIL" };
                        let ms = micros as f64 / 1000.0;
                        let body = format!("NET_DIAG PORT [{status}] {target} -> {desc} | time: {ms:.3} ms");
                        state.last_report = Some(body.clone());
                        Some(JfpMessage::reply(status, body))
                    }
                }
                "help" => Some(JfpMessage::reply(
                    "OK",
                    "NET_DIAG Help:\n\
                     - check           : check XCRATESX_TCP_TARGETS (default: localhost ports 22 and 443)\n\
                     - ping <ip:port>  : TCP reachability and latency in microseconds\n\
                     - port <ip> <nr>  : check a TCP port\n\
                     - help            : show this help",
                )),
                other => {
                    Some(JfpMessage::reply(
                        "ERROR",
                        format!("NET_DIAG | Unknown command: '{other}'. Enter 'help'."),
                    ))
                }
            }
        }
        "TICK" => {
            state.check_count += 1;
            if let Some(report) = &state.last_report {
                Some(JfpMessage::reply(
                    "OK",
                    format!(
                        "NET_DIAG (Tick #{}) | Last result:\n{report}",
                        state.check_count
                    ),
                ))
            } else {
                Some(JfpMessage::reply(
                    "OK",
                    format!(
                        "NET_DIAG (Tick #{}) | Ready. Enter 'check' to test configured targets.",
                        state.check_count
                    ),
                ))
            }
        }
        _ => None,
    }
}

fn main() {
    let stdin = io::stdin();
    let mut reader = JfpReader::new(stdin.lock());
    let mut stdout = io::stdout();
    let mut state = State::default();

    while let Some(frame) = reader.next_frame() {
        let Some(message) = handle(&frame, &mut state) else {
            continue;
        };

        if writeln!(stdout, "{}", message.to_jfp_string()).is_err() {
            break;
        }

        if stdout.flush().is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_command_help() {
        let req = JfpMessage::request("COMMAND", "help");
        let mut state = State::default();
        let resp = handle(&req, &mut state).expect("response");
        assert_eq!(resp.status, "OK");
        assert!(resp.body.contains("NET_DIAG Help"));
    }

    #[test]
    fn test_handle_command_ping_syntax() {
        let req = JfpMessage::request("COMMAND", "ping 127.0.0.1:22");
        let mut state = State::default();
        let resp = handle(&req, &mut state).expect("response");
        assert!(resp.body.contains("NET_DIAG PING"));
    }

    #[test]
    fn test_tick_increments_counter() {
        let req = JfpMessage::request("TICK", "1");
        let mut state = State::default();
        let resp1 = handle(&req, &mut state).expect("resp1");
        assert!(resp1.body.contains("Tick #1"));
        let resp2 = handle(&req, &mut state).expect("resp2");
        assert!(resp2.body.contains("Tick #2"));
    }
}
