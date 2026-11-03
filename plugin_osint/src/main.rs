use std::io::{self, Write};
use std::thread::sleep;
use std::time::Duration;

use shared_contract::{JfpMessage, JfpReader};

const TUNNEL_TICKS: u64 = 10;

#[derive(Default)]
struct OsintState {
    target: Option<String>,
}

fn main() {
    let stdin = io::stdin();
    let mut reader = JfpReader::new(stdin.lock());
    let mut stdout = io::stdout();
    let mut state = OsintState::default();

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

fn handle(frame: &JfpMessage, state: &mut OsintState) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let target = frame.body.trim();

            if !target.is_empty() {
                state.target = Some(target.to_owned());
            }

            Some(JfpMessage::reply("OK", "Przyjeto komende"))
        }
        "TICK" => {
            let tick = frame.body.trim().parse().ok()?;

            sleep(Duration::from_millis(50));

            Some(respond(tick, state))
        }
        _ => None,
    }
}

fn respond(tick: u64, state: &OsintState) -> JfpMessage {
    if tick <= TUNNEL_TICKS {
        return JfpMessage::reply(
            "WAIT",
            format!("OSINT: Zestawianie tuneli proxy [{tick}/{TUNNEL_TICKS}]"),
        );
    }

    let mut body = String::from(
        "CEL: Zlokalizowany | Wektory: 3 aktywne | Scrapowanie ukryte w toku...",
    );

    if let Some(target) = &state.target {
        body = format!("TRYB: SKANOWANIE CELU: {target} | {body}");
    }

    JfpMessage::reply("OK", body)
}
