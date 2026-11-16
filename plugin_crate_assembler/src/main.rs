use std::io::{self, Write};
use std::process::Command;

use shared_contract::{JfpMessage, JfpReader};

#[derive(Default)]
struct State {
    last_result: Option<String>,
}

fn build_crate(crate_name: &str, _version: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(format!("crates/{crate_name}/Cargo.toml"));
    if let Some(_v) = _version {
        cmd.arg("-p").arg(crate_name);
    }
    let output = cmd.output().map_err(|e| format!("Failed: {e}"))?;
    if output.status.success() {
        Ok(format!("BUILD OK: {} {}", crate_name, _version.unwrap_or("latest")))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("BUILD FAILED: {} - {}", crate_name, stderr))
    }
}

fn assemble_configuration(config: &str) -> Result<String, String> {
    let lines: Vec<&str> = config.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let mut results = Vec::new();
    for line in lines {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() == 2 {
            let (name, version) = (parts[0].trim(), parts[1].trim());
            let result = build_crate(name, Some(version));
            results.push(result.unwrap_or_else(|e| format!("{name}: {e}")));
        }
    }
    Ok(format!("Assembly results:\n{}", results.join("\n")))
}

fn handle(frame: &JfpMessage, state: &mut State) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let input = frame.body.trim();
            if input.is_empty() {
                return Some(JfpMessage::reply("OK", "Podaj konfigurację: 'test:serde:1, serde_json:1'"));
            }
            if input.starts_with("test:") {
                let cfg = &input[5..].trim();
                match assemble_configuration(cfg) {
                    Ok(result) => {
                        state.last_result = Some(result.clone());
                        return Some(JfpMessage::reply("OK", result));
                    }
                    Err(e) => {
                        state.last_result = Some(e.clone());
                        return Some(JfpMessage::reply("ERROR", e));
                    }
                }
            }
            if let Some(arg) = input.split_whitespace().nth(1) {
                match build_crate(arg, None) {
                    Ok(result) => {
                        state.last_result = Some(result.clone());
                        return Some(JfpMessage::reply("OK", result));
                    }
                    Err(err) => {
                        state.last_result = Some(err.clone());
                        return Some(JfpMessage::reply("ERROR", err));
                    }
                }
            }
            Some(JfpMessage::reply("OK", format!("Plugin | '{input}' akceptowane. Uzyj 'test:<cfg>' dla skladania.")))
        }
        "TICK" => {
            if let Some(out) = &state.last_result {
                Some(JfpMessage::reply("OK", format!("CRATE_ASSEMBLER | {out}")))
            } else {
                Some(JfpMessage::reply("OK", "CRATE_ASSEMBLER | Gotowy. Wpisuj: test:serde:1, serde_json:1"))
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
        let Some(message) = handle(&frame, &mut state) else { continue; };
        if writeln!(stdout, "{}", message.to_jfp_string()).is_err() { break; }
        if stdout.flush().is_err() { break; }
    }
}
