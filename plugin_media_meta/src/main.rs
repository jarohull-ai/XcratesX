use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use lofty::prelude::*;
use lofty::probe::Probe;
use shared_contract::{JfpMessage, JfpReader};

#[derive(Default)]
struct State {
    last_output: Option<String>,
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let min = total_secs / 60;
    let sec = total_secs % 60;
    format!("{min}:{sec:02}")
}

fn analyze_file(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Enter an audio file path.".into());
    }

    let p = Path::new(path);
    if !p.is_file() {
        return Err(format!("Path is missing or not a file: {path}"));
    }

    let tagged_file = Probe::open(p)
        .map_err(|e| format!("Cannot open: {e}"))?
        .read()
        .map_err(|e| format!("Cannot read: {e}"))?;

    let mut out = format!("MEDIA META | File: {path}\n\n");

    let file_type = tagged_file.file_type();
    let _ = writeln!(out, "  Format:     {file_type:?}");

    let props = tagged_file.properties();
    let _ = writeln!(
        out,
        "  Duration:       {}",
        format_duration(props.duration())
    );

    if let Some(sr) = props.sample_rate() {
        let channels = props.channels().unwrap_or(0);
        let ch_label = match channels {
            1 => "Mono",
            2 => "Stereo",
            _ => "Multi",
        };
        let _ = writeln!(out, "  Sample:     {sr} Hz / {ch_label}");
    }

    if let Some(bitrate) = props.audio_bitrate() {
        let _ = writeln!(out, "  Bitrate:    {bitrate} kbps");
    }

    if let Some(bps) = props.bit_depth() {
        let _ = writeln!(out, "  Bit depth:  {bps}-bit");
    }

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    if let Some(tag) = tag {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  Title:      {}",
            tag.title().as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Artist:    {}",
            tag.artist().as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Album:      {}",
            tag.album().as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Year:        {}",
            tag.year().map(|y| y.to_string()).unwrap_or("-".into())
        );
        let _ = writeln!(out, "  Genre:    {}", tag.genre().as_deref().unwrap_or("-"));

        let pics = tag.pictures();
        if pics.is_empty() {
            let _ = writeln!(out, "  Cover:      No");
        } else {
            let pic = &pics[0];
            let size = pic.data().len();
            let _ = writeln!(
                out,
                "  Cover:      Yes ({}, {} KB)",
                pic.mime_type().unwrap_or(&lofty::picture::MimeType::Png),
                size / 1024
            );
        }
    } else {
        let _ = writeln!(out, "\n  (no tags)");
    }

    Ok(out)
}

fn handle(frame: &JfpMessage, state: &mut State) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let path = frame.body.trim();
            match analyze_file(path) {
                Ok(report) => {
                    state.last_output = Some(report.clone());
                    Some(JfpMessage::reply("OK", report))
                }
                Err(err) => {
                    let msg = format!("MEDIA META | Error: {err}");
                    state.last_output = Some(msg.clone());
                    Some(JfpMessage::reply("ERROR", msg))
                }
            }
        }
        "TICK" => {
            if let Some(output) = &state.last_output {
                Some(JfpMessage::reply("OK", output.as_str()))
            } else {
                Some(JfpMessage::reply(
                    "OK",
                    "MEDIA META | Enter an audio file path (MP3, FLAC, WAV, OGG...)",
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
