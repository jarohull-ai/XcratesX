use std::io::{self, Write};

use shared_contract::{JfpMessage, JfpReader};

const PARAGRAPHS: usize = 10;
const DEFAULT_TITLE: &str = "WLADCY SWIATA";

const PASSAGE: &str = "Wladcy swiata nie dzielili sie wladza, dzielili sie tylko mapami. W komnatach bez okien podpisywali traktaty, ktorych nikt nie czytal do konca, a granice przesuwali o milimetry, ktore na papierze znaczylo prowincje. Kupcy przynosili im sol, jedwab i wiesci, oni oddawali w zamian prawo do ciszy. Gdy zapadal zmierzk nad portem, latarnie zapalaly sie jedna po drugiej, jakby ktos odliczal dni do konca porzadku, ktory sami ustanowili.";

struct WritingState {
    title: String,
}

impl Default for WritingState {
    fn default() -> Self {
        Self {
            title: String::from(DEFAULT_TITLE),
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let mut reader = JfpReader::new(stdin.lock());
    let mut stdout = io::stdout();
    let mut state = WritingState::default();

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

fn handle(frame: &JfpMessage, state: &mut WritingState) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let title = frame.body.trim();

            if !title.is_empty() {
                state.title = title.to_owned();
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

fn respond(tick: u64, state: &WritingState) -> JfpMessage {
    let mut body = format!("ROZDZIAL {tick:03} :: {}\n", state.title);

    for paragraph in 0..PARAGRAPHS {
        body.push_str(&format!("[{:02}] ", paragraph + 1));
        body.push_str(PASSAGE);
        body.push('\n');
    }

    body.push_str(&format!("--- koniec przekazu {tick:03} ---"));

    JfpMessage::reply("OK", body)
}
