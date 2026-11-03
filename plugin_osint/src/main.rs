use std::io::{self, Write};
use std::sync::Arc;

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

async fn scan_target(target: String, state: Arc<Mutex<OsintState>>) {
    let url = format!("http://ip-api.com/json/{target}");

    let result = reqwest::get(&url).await;

    let mut guard = state.lock().await;

    if guard.target != target {
        return;
    }

    match result {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(json) => {
                let formatted = serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string());
                guard.scan = ScanState::Finished(formatted);
            }
            Err(err) => {
                guard.scan = ScanState::Error(format!("Blad parsowania JSON: {err}"));
            }
        },
        Err(err) => {
            guard.scan = ScanState::Error(format!("Blad HTTP: {err}"));
        }
    }
}

fn handle_command(target: &str, state: &mut OsintState) -> JfpMessage {
    let target = target.trim();

    if target.is_empty() {
        return JfpMessage::reply("OK", "OSINT | Pusty cel - podaj adres IP lub domene.");
    }

    state.target = target.to_owned();
    state.scan = ScanState::Scanning;

    JfpMessage::reply("OK", format!("OSINT | Cel zablokowany: {target}. Rozpoczynam skanowanie..."))
}

fn handle_tick(state: &OsintState) -> JfpMessage {
    match &state.scan {
        ScanState::Idle => JfpMessage::reply(
            "OK",
            "OSINT | System gotowy. Podaj cel w wierszu polecen (np. 8.8.8.8)",
        ),
        ScanState::Scanning => JfpMessage::reply(
            "WAIT",
            format!(
                "OSINT | Cel zablokowany: {}. Asynchroniczne skanowanie w toku...",
                state.target
            ),
        ),
        ScanState::Finished(data) => JfpMessage::reply(
            "OK",
            format!("OSINT | Raport dla {}:\n{data}", state.target),
        ),
        ScanState::Error(err) => JfpMessage::reply(
            "ERROR",
            format!("OSINT | Blad dla {}: {err}", state.target),
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
