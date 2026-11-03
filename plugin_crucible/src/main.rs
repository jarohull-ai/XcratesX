use std::io::{self, Write};

use shared_contract::{JfpMessage, JfpReader};

const PAYLOAD_LEN: usize = 500_000;
const CRASH_TICK: u64 = 6;

fn main() {
    let stdin = io::stdin();
    let mut reader = JfpReader::new(stdin.lock());
    let mut stdout = io::stdout();

    while let Some(frame) = reader.next_frame() {
        match frame.intent.as_deref() {
            Some("COMMAND") => {
                let message = JfpMessage::reply("OK", "Przyjeto komende");

                if writeln!(stdout, "{}", message.to_jfp_string()).is_err() {
                    break;
                }

                if stdout.flush().is_err() {
                    break;
                }
            }
            Some("TICK") => {
                let Some(tick) = frame.body.trim().parse::<u64>().ok() else {
                    continue;
                };

                if tick >= CRASH_TICK {
                    print!(
                        "[JFP_HEADER]\nVERSION: v5.4.0\nSTATUS: ERROR\n[MESSAGE_BODY]\nCRITICAL: KERNEL PANIC - MEMORY CORRUPTION"
                    );
                    stdout.flush().unwrap();
                    std::process::exit(1);
                }

                let message = JfpMessage::reply("OK", "A".repeat(PAYLOAD_LEN));

                if writeln!(stdout, "{}", message.to_jfp_string()).is_err() {
                    break;
                }

                if stdout.flush().is_err() {
                    break;
                }
            }
            _ => {}
        }
    }
}
