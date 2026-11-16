use std::io::{self, Write};

use shared_contract::{JfpMessage, JfpReader};

struct State {
    last_output: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self { last_output: None }
    }
}

fn handle(frame: &JfpMessage, state: &mut State) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let input = frame.body.trim();

            // TODO: implement your plugin logic here
            state.last_output = Some(format!("Received: {input}"));

            Some(JfpMessage::reply(
                "OK",
                format!("PLUGIN | Processed: {input}"),
            ))
        }
        "TICK" => {
            if let Some(output) = &state.last_output {
                Some(JfpMessage::reply("OK", format!("PLUGIN | {output}")))
            } else {
                Some(JfpMessage::reply(
                    "OK",
                    "PLUGIN | Ready. Enter a command.",
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
