use std::hint::black_box;
use std::io::{self, Write};

use shared_contract::{JfpMessage, JfpReader};

const DSP_ITERATIONS: u64 = 1_000_000;
const DEFAULT_LOAD: u64 = 87;

struct AudioState {
    load: u64,
    mode: Option<String>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            load: DEFAULT_LOAD,
            mode: None,
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let mut reader = JfpReader::new(stdin.lock());
    let mut stdout = io::stdout();
    let mut state = AudioState::default();

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

fn handle(frame: &JfpMessage, state: &mut AudioState) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let command = frame.body.trim();

            if let Ok(load) = command.parse::<u64>() {
                state.load = load.min(100);
            } else if !command.is_empty() {
                state.mode = Some(command.to_owned());
            }

            Some(JfpMessage::reply("OK", "Przyjeto komende"))
        }
        "TICK" => {
            frame.body.trim().parse::<u64>().ok()?;

            burn_cpu();

            Some(respond(state))
        }
        _ => None,
    }
}

fn burn_cpu() {
    let mut accumulator = 0.0_f64;

    for index in 1..=DSP_ITERATIONS {
        let value = index as f64;
        accumulator += (value.sin() * value.cos() + value.sqrt()).abs();
    }

    black_box(accumulator);
}

fn respond(state: &AudioState) -> JfpMessage {
    let mode = match &state.mode {
        Some(mode) => format!(" | Tryb: {mode}"),
        None => String::new(),
    };

    JfpMessage::reply(
        "OK",
        format!(
            "AUDIO DSP STUDIO | Obciazenie bufora: {}% | Latencja: 2.1ms | Ograniczenie przesterowania: Aktywne{mode}",
            state.load
        ),
    )
}
