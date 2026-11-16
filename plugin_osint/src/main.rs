use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use shared_contract::{JfpMessage, JfpReader};
use tokio::sync::{mpsc, Mutex};

enum ScanState {
    Idle,
    Scanning,
    Finished(String),
    Error(String),
}

struct OsintState {
    target: String,
    scan: ScanState,
}

impl Default for OsintState {
    fn default() -> Self {
        Self {
            target: String::new(),
            scan: ScanState::Idle,
        }
    }
}

fn send_reply(stdout: &mut io::Stdout, message: JfpMessage) -> bool {
    if writeln!(stdout, "{}", message.to_jfp_string()).is_err() {
        return false;
    }
    stdout.flush().is_ok()
}

fn is_valid_target(target: &str) -> bool {
    !target.is_empty()
        && target.len() <= 253
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')
}

async fn scan_target(target: String, state: Arc<Mutex<OsintState>>) {
    let url = format!("https://ip-api.com/json/{target}");

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let mut guard = state.lock().await;
            if guard.target == target {
                guard.scan = ScanState::Error(format!("HTTP client configuration error: {e}"));
            }
            return;
        }
    };

    let result = client.get(&url).send().await;

    let mut guard = state.lock().await;

    if guard.target != target {
        return;
    }

    match result {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(json) => {
                let formatted =
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string());
                guard.scan = ScanState::Finished(formatted);
            }
            Err(err) => {
                guard.scan = ScanState::Error(format!("JSON parsing failed: {err}"));
            }
        },
        Err(err) => {
            guard.scan = ScanState::Error(format!("HTTP error (timeout/connection): {err}"));
        }
    }
}

fn handle_command(target: &str, state: &mut OsintState) -> JfpMessage {
    let target = target.trim();

    if target.is_empty() {
        return JfpMessage::reply("OK", "OSINT | Enter an IP address or domain.");
    }

    if !is_valid_target(target) {
        return JfpMessage::reply(
            "ERROR",
            "OSINT | Invalid target (letters, digits, dots, hyphens and IPv6 colons only).",
        );
    }

    state.target = target.to_owned();
    state.scan = ScanState::Scanning;

    JfpMessage::reply(
        "OK",
        format!("OSINT | Target: {target}. Starting lookup..."),
    )
}

fn handle_tick(state: &OsintState) -> JfpMessage {
    match &state.scan {
        ScanState::Idle => {
            JfpMessage::reply("OK", "OSINT | Ready. Enter an IP or domain (e.g. 8.8.8.8)")
        }
        ScanState::Scanning => JfpMessage::reply(
            "WAIT",
            format!("OSINT | Target: {}. Lookup in progress...", state.target),
        ),
        ScanState::Finished(data) => JfpMessage::reply(
            "OK",
            format!("OSINT | Report for {}:\n{data}", state.target),
        ),
        ScanState::Error(err) => JfpMessage::reply(
            "ERROR",
            format!("OSINT | Error for {}: {err}", state.target),
        ),
    }
}

#[tokio::main]
async fn main() {
    let state = Arc::new(Mutex::new(OsintState::default()));

    let (tx, mut rx) = mpsc::unbounded_channel::<JfpMessage>();

    tokio::task::spawn_blocking(move || {
        let stdin = io::stdin();
        let mut reader = JfpReader::new(stdin.lock());

        while let Some(frame) = reader.next_frame() {
            if tx.send(frame).is_err() {
                break;
            }
        }
    });

    let mut stdout = io::stdout();

    while let Some(frame) = rx.recv().await {
        let intent = frame.intent.as_deref();

        match intent {
            Some("COMMAND") => {
                let reply = {
                    let mut guard = state.lock().await;
                    handle_command(&frame.body, &mut guard)
                };

                if !send_reply(&mut stdout, reply) {
                    break;
                }

                let target = state.lock().await.target.clone();
                let state_clone = Arc::clone(&state);
                tokio::spawn(scan_target(target, state_clone));
            }
            Some("TICK") => {
                let reply = {
                    let guard = state.lock().await;
                    handle_tick(&guard)
                };

                if !send_reply(&mut stdout, reply) {
                    break;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_target() {
        assert!(is_valid_target("8.8.8.8"));
        assert!(is_valid_target("github.com"));
        assert!(is_valid_target("2001:4860:4860::8888"));
        assert!(!is_valid_target(""));
        assert!(!is_valid_target("8.8.8.8; rm -rf /"));
        assert!(!is_valid_target("http://malicious.com"));
        assert!(!is_valid_target("example.com?query=1"));
    }
}
