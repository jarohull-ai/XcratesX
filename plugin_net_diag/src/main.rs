use std::io::{self, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use shared_contract::{JfpMessage, JfpReader};

struct State {
    last_report: Option<String>,
    check_count: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_report: None,
            check_count: 0,
        }
    }
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
                (false, 0, "Brak prawidlowego adresu IP".to_string())
            }
        }
        Err(e) => (false, 0, format!("Blad rozwiazywania adresu: {e}")),
    }
}

fn run_cluster_diagnostic() -> String {
    let mut out = String::new();
    out.push_str("=== DIAGNOSTYKA WEZLOW KLASTRA (Dual-Node JFP) ===\n");

    let targets = [
        ("Viper Local (LM Studio 9B)", "127.0.0.1:1234"),
        ("Venom Tunel (LM Studio 2B)", "127.0.0.1:1235"),
        ("Venom Kabel (SSH 10.42.0.2:22)", "10.42.0.2:22"),
        ("Viper Kabel (SSH 10.42.0.1:22)", "10.42.0.1:22"),
    ];

    for (name, addr) in targets {
        let (ok, micros, desc) = check_tcp_port(addr, Duration::from_millis(500));
        let badge = if ok { "[OK]   " } else { "[FAIL] " };
        let latency_ms = micros as f64 / 1000.0;
        out.push_str(&format!("{badge} {name:<32} -> {desc} (latencja: {latency_ms:.2} ms)\n"));
    }

    out.push_str("\nKomendy: check | ping <ip>:<port> | port <ip> <port> | help");
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
                    "NET_DIAG | Wpisz komende: check, ping <host:port>, port <host> <port>, help",
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
                        Some(JfpMessage::reply("ERROR", "Uzycie: ping <host:port>"))
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
                        let body = format!("NET_DIAG PING [{status}] {addr} -> {desc} | czas: {ms:.3} ms");
                        state.last_report = Some(body.clone());
                        Some(JfpMessage::reply(status, body))
                    }
                }
                "port" => {
                    if parts.len() < 3 {
                        Some(JfpMessage::reply("ERROR", "Uzycie: port <host> <port>"))
                    } else {
                        let host = parts[1];
                        let port = parts[2];
                        let target = format!("{host}:{port}");
                        let (ok, micros, desc) = check_tcp_port(&target, Duration::from_millis(800));
                        let status = if ok { "OK" } else { "FAIL" };
                        let ms = micros as f64 / 1000.0;
                        let body = format!("NET_DIAG PORT [{status}] {target} -> {desc} | czas: {ms:.3} ms");
                        state.last_report = Some(body.clone());
                        Some(JfpMessage::reply(status, body))
                    }
                }
                "help" => Some(JfpMessage::reply(
                    "OK",
                    "NET_DIAG Pomoc:\n\
                     - check           : pelna diagnostyka lacza Viper <-> Venom i portow LM Studio\n\
                     - ping <ip:port>  : test reachability i latencji w mikrosekundach\n\
                     - port <ip> <nr>  : szybki test danego portu TCP\n\
                     - help            : ten komunikat",
                )),
                other => {
                    Some(JfpMessage::reply(
                        "ERROR",
                        format!("NET_DIAG | Nieznana komenda: '{other}'. Wpisz 'help'."),
                    ))
                }
            }
        }
        "TICK" => {
            state.check_count += 1;
            if let Some(report) = &state.last_report {
                Some(JfpMessage::reply(
                    "OK",
                    format!("NET_DIAG (Tick #{}) | Ostatni wynik:\n{report}", state.check_count),
                ))
            } else {
                Some(JfpMessage::reply(
                    "OK",
                    format!(
                        "NET_DIAG (Tick #{}) | Lacze aktywne. Wpisz 'check' aby przeskanowac klaster.",
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
        assert!(resp.body.contains("NET_DIAG Pomoc"));
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
