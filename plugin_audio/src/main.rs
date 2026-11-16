use std::fs;
use std::io::{self, Write};
use std::path::Path;

use shared_contract::{JfpMessage, JfpReader};

#[derive(Default)]
struct AudioState {
    last_report: Option<String>,
    last_error: Option<String>,
}

const MAX_WAV_BYTES: u64 = 500 * 1024 * 1024; // 500 MB limit

fn analyze_wav(path: &Path) -> Result<String, String> {
    if !path.is_file() {
        return Err("Sciezka nie istnieje lub nie jest zwyklym plikiem.".into());
    }

    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > MAX_WAV_BYTES {
            return Err(format!(
                "Plik przekracza limit bezpieczenstwa ({} MB > 500 MB).",
                meta.len() / (1024 * 1024)
            ));
        }
    }

    let reader = hound::WavReader::open(path).map_err(|e| format!("Nie mozna otworzyc: {e}"))?;

    let spec = reader.spec();
    let num_samples = reader.len() as u64;
    let channels = spec.channels as u64;
    let sample_rate = spec.sample_rate as u64;
    let samples_per_channel = num_samples.checked_div(channels).unwrap_or(0);
    let duration_secs = if sample_rate > 0 {
        samples_per_channel as f64 / sample_rate as f64
    } else {
        0.0
    };

    let min = duration_secs as u64 / 60;
    let sec = duration_secs as u64 % 60;
    let ms = ((duration_secs.fract()) * 1000.0) as u64;

    let mut report = String::new();
    report.push_str(&format!("PLIK: {}\n", path.display()));
    report.push_str(&format!("FORMAT: {:?}\n", spec.sample_format));
    report.push_str(&format!("KANALY: {}\n", spec.channels));
    report.push_str(&format!("SAMPLE RATE: {} Hz\n", spec.sample_rate));
    report.push_str(&format!("BITY: {}\n", spec.bits_per_sample));
    report.push_str(&format!(
        "PROBKI: {} (na kanal: {})\n",
        num_samples, samples_per_channel
    ));
    report.push_str(&format!("CZAS: {min:02}:{sec:02}.{ms:03}"));

    let peak_rms = compute_peak_rms(path, &spec);
    if let Some((peak, rms)) = peak_rms {
        report.push_str(&format!("\nPEAK: {peak:.4}"));
        report.push_str(&format!("\nRMS:  {rms:.4}"));

        if peak > 0.0 && rms > 0.0 {
            let crest_db = 20.0 * (peak / rms).log10();
            report.push_str(&format!("\nCREST: {crest_db:.1} dB"));
        }
    }

    Ok(report)
}

fn compute_peak_rms(path: &Path, spec: &hound::WavSpec) -> Option<(f64, f64)> {
    let reader = hound::WavReader::open(path).ok()?;

    if reader.len() == 0 {
        return None;
    }

    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f64;
            let mut peak: f64 = 0.0;
            let mut sum_sq: f64 = 0.0;
            let mut count: u64 = 0;

            for s in reader.into_samples::<i32>().flatten() {
                let normalized = s as f64 / max_val;
                peak = peak.max(normalized.abs());
                sum_sq += normalized * normalized;
                count += 1;
            }

            if count == 0 {
                return None;
            }

            let rms = (sum_sq / count as f64).sqrt();
            Some((peak, rms))
        }
        hound::SampleFormat::Float => {
            let mut peak: f64 = 0.0;
            let mut sum_sq: f64 = 0.0;
            let mut count: u64 = 0;

            for s in reader.into_samples::<f32>().flatten() {
                let s = s as f64;
                peak = peak.max(s.abs());
                sum_sq += s * s;
                count += 1;
            }

            if count == 0 {
                return None;
            }

            let rms = (sum_sq / count as f64).sqrt();
            Some((peak, rms))
        }
    }
}

fn handle(frame: &JfpMessage, state: &mut AudioState) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let path_str = frame.body.trim();

            if path_str.is_empty() {
                return Some(JfpMessage::reply(
                    "OK",
                    "AUDIO | Podaj sciezke do pliku WAV.",
                ));
            }

            let path = Path::new(path_str);

            match analyze_wav(path) {
                Ok(report) => {
                    state.last_report = Some(report.clone());
                    state.last_error = None;
                    Some(JfpMessage::reply(
                        "OK",
                        format!("AUDIO | Analiza zakonczona:\n{report}"),
                    ))
                }
                Err(err) => {
                    state.last_error = Some(err.clone());
                    state.last_report = None;
                    Some(JfpMessage::reply("ERROR", format!("AUDIO | {err}")))
                }
            }
        }
        "TICK" => {
            if let Some(report) = &state.last_report {
                Some(JfpMessage::reply(
                    "OK",
                    format!("AUDIO | Ostatnia analiza:\n{report}"),
                ))
            } else if let Some(err) = &state.last_error {
                Some(JfpMessage::reply(
                    "ERROR",
                    format!("AUDIO | Ostatni blad: {err}"),
                ))
            } else {
                Some(JfpMessage::reply(
                    "OK",
                    "AUDIO | Gotowy. Podaj sciezke do pliku WAV w wierszu polecen.",
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
