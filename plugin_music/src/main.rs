use std::io::{self, Write};

use shared_contract::{JfpMessage, JfpReader};

const DEFAULT_TRACK: &str = "SOFIA IGNIS - Epic Choral";

struct MusicState {
    track: String,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            track: String::from(DEFAULT_TRACK),
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let mut reader = JfpReader::new(stdin.lock());
    let mut stdout = io::stdout();
    let mut state = MusicState::default();

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

fn handle(frame: &JfpMessage, state: &mut MusicState) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let command = frame.body.trim();

            let requested = command
                .strip_prefix("PLAY ")
                .or_else(|| command.strip_prefix("play "))
                .unwrap_or(command);

            if !requested.is_empty() {
                state.track = requested.to_owned();
            }

            Some(JfpMessage::reply("OK", "Przyjeto komende"))
        }
        "TICK" => {
            let tick = frame.body.trim().parse().ok()?;

            Some(respond(tick, state))
        }
        _ => None,
    }
}

fn respond(tick: u64, state: &MusicState) -> JfpMessage {
    let time = tick * 15;
    let min = time / 60_000;
    let sec = (time / 1_000) % 60;
    let ms = time % 1_000;

    JfpMessage::reply(
        "OK",
        format!(
            "PLAYING: {} | TIME: {min:02}:{sec:02}:{ms:03} | BPM: 124.5 | FFT: ▂▃▅▆█▆▅▃▂",
            state.track
        ),
    )
}
