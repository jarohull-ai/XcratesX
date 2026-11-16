use shared_contract::{JfpMessage, JfpReader};
pub mod experiments;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub fn dependencies(input: &str) -> Result<BTreeMap<String, String>, String> {
    let mut deps = BTreeMap::new();
    for item in input
        .split([',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let (name, version) = item.split_once(':').unwrap_or((item, "*"));
        let (name, version) = (name.trim(), version.trim());
        if name.is_empty()
            || name.len() > 64
            || !name.starts_with(|c: char| c.is_ascii_alphabetic())
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!("Invalid crate name: {name:?}"));
        }
        semver::VersionReq::parse(version)
            .map_err(|e| format!("Invalid version for {name}: {e}"))?;
        if deps.insert(name.to_owned(), version.to_owned()).is_some() {
            return Err(format!("Duplicate crate: {name}"));
        }
    }
    if deps.is_empty() || deps.len() > 32 {
        return Err("Provide 1–32 crates: serde:1, serde_json:1".into());
    }
    Ok(deps)
}
fn project(deps: &BTreeMap<String, String>) -> Result<tempfile::TempDir, String> {
    let dir = tempfile::Builder::new()
        .prefix("xcratesx-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let manifest = format!("[package]\nname = \"xcratesx_experiment\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n[dependencies]\n{}", toml::to_string(deps).map_err(|e| e.to_string())?);
    fs::write(dir.path().join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
    fs::create_dir(dir.path().join("src")).map_err(|e| e.to_string())?;
    fs::write(
        dir.path().join("src/lib.rs"),
        "// Dependency compilation experiment.\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(dir)
}
fn cargo(path: &Path, args: &[&str]) -> Result<String, String> {
    let stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
    let stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut command = Command::new("cargo");
    command
        .args(args)
        .arg("--manifest-path")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
        .stderr(stderr.try_clone().map_err(|e| e.to_string())?);
    if std::env::var_os("XCRATESX_OFFLINE").is_some() {
        command.arg("--offline");
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start cargo: {e}"))?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        let oversized = [&stdout, &stderr].iter().any(|f| {
            f.metadata()
                .map(|m| m.len() > 8 * 1024 * 1024)
                .unwrap_or(true)
        });
        if start.elapsed() > Duration::from_secs(180) || oversized {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cargo exceeded 180 seconds or 8 MiB output".into());
        }
        thread::sleep(Duration::from_millis(50));
    };
    fn read(mut file: fs::File) -> Result<String, String> {
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut text = String::new();
        file.take(8 * 1024 * 1024)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        Ok(text)
    }
    if !status.success() {
        return Err(format!("Cargo failed ({status}):\n{}", read(stderr)?));
    }
    read(stdout)
}
pub fn assemble(input: &str) -> Result<String, String> {
    if input == "@saved" {
        return experiments::run_saved(false);
    }
    let deps = dependencies(input)?;
    let dir = project(&deps)?;
    cargo(&dir.path().join("Cargo.toml"), &["build", "--quiet"])?;
    let lock = fs::read_to_string(dir.path().join("Cargo.lock")).map_err(|e| e.to_string())?;
    Ok(format!("BUILD OK: dependencies compiled together. Application behavior was not tested.\n\nRequested:\n{}\n\nResolved lockfile:\n{lock}", deps.iter().map(|(n,v)| format!("{n}: {v}")).collect::<Vec<_>>().join("\n")))
}
fn graph(raw: &str) -> Result<String, String> {
    let data: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let packages = data["packages"].as_array().ok_or("Missing packages")?;
    let mut labels = BTreeMap::new();
    for p in packages {
        labels.insert(
            p["id"].as_str().ok_or("Missing package id")?,
            format!(
                "{}@{}",
                p["name"].as_str().unwrap_or("?"),
                p["version"].as_str().unwrap_or("?")
            ),
        );
    }
    let nodes = data["resolve"]["nodes"]
        .as_array()
        .ok_or("Missing resolved graph")?;
    let mut rows = Vec::new();
    let mut edges = 0;
    for node in nodes {
        let id = node["id"].as_str().ok_or("Missing node id")?;
        let deps = node["dependencies"]
            .as_array()
            .ok_or("Missing dependencies")?;
        edges += deps.len();
        let mut children = deps
            .iter()
            .filter_map(|d| d.as_str())
            .map(|id| labels.get(id).cloned().unwrap_or_else(|| id.to_owned()))
            .collect::<Vec<_>>();
        children.sort();
        rows.push(format!(
            "{} -> {}",
            labels.get(id).cloned().unwrap_or_else(|| id.to_owned()),
            if children.is_empty() {
                "(none)".into()
            } else {
                children.join(", ")
            }
        ));
    }
    rows.sort();
    Ok(format!(
        "Resolved packages: {}; dependency edges: {edges}\n\n{}",
        packages.len(),
        rows.join("\n")
    ))
}
pub fn analyze(input: &str) -> Result<String, String> {
    if input == "@saved" {
        return experiments::run_saved(true);
    }
    if let Some(path) = input.strip_prefix("path ") {
        let manifest = fs::canonicalize(path.trim()).map_err(|e| e.to_string())?;
        return graph(&cargo(
            &manifest,
            &["metadata", "--format-version", "1", "--locked"],
        )?);
    }
    let dir = project(&dependencies(input)?)?;
    graph(&cargo(
        &dir.path().join("Cargo.toml"),
        &["metadata", "--format-version", "1"],
    )?)
}
/// One background experiment; ticks remain responsive.
pub fn serve(prefix: &'static str, action: fn(&str) -> Result<String, String>) {
    let mut reader = JfpReader::new(io::stdin().lock());
    let mut stdout = io::stdout();
    let (tx, rx) = mpsc::channel();
    let mut busy = false;
    let mut last = JfpMessage::reply("OK", format!("Use: {prefix}serde:1, serde_json:1"));
    while let Some(frame) = reader.next_frame() {
        if let Ok(result) = rx.try_recv() {
            last = result;
            busy = false;
        }
        let reply = match frame.intent.as_deref() {
            Some("TICK") => last.clone(),
            Some("COMMAND") if busy => JfpMessage::reply("BUSY", "An experiment is still running."),
            Some("COMMAND") => {
                if let Some(input) = frame.body.trim().strip_prefix(prefix) {
                    let input = input.to_owned();
                    let tx = tx.clone();
                    busy = true;
                    last = JfpMessage::reply(
                        "BUSY",
                        "Cargo experiment running (timeout: 180 seconds)...",
                    );
                    thread::spawn(move || {
                        let mut result = match action(&input) {
                            Ok(body) => JfpMessage::reply("OK", body),
                            Err(body) => JfpMessage::reply("ERROR", body),
                        };
                        if result.body.len() > 128 * 1024 {
                            let mut end = 128 * 1024;
                            while !result.body.is_char_boundary(end) {
                                end -= 1;
                            }
                            result.body.truncate(end);
                            result.body.push_str("\n[Output truncated at 128 KiB]");
                        }
                        let _ = tx.send(result);
                    });
                    last.clone()
                } else {
                    JfpMessage::reply("ERROR", format!("Use: {prefix}serde:1, serde_json:1"))
                }
            }
            _ => continue,
        };
        if writeln!(stdout, "{}", reply.to_jfp_string())
            .and_then(|_| stdout.flush())
            .is_err()
        {
            break;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_versions_and_names() {
        let deps = dependencies("serde:1, serde_json:=1.0.100\ntokio").unwrap();
        assert_eq!(deps["serde_json"], "=1.0.100");
        assert_eq!(deps["tokio"], "*");
        for bad in [
            "",
            "../escape:1",
            "serde:bad",
            "serde:1,serde:2",
            "a\"b:1",
            "serde:",
        ] {
            assert!(dependencies(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn generated_manifest_preserves_requirements() {
        let dir = project(&dependencies("serde:1").unwrap()).unwrap();
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(dir.path().join("Cargo.toml")).unwrap()).unwrap();
        assert_eq!(manifest["dependencies"]["serde"].as_str(), Some("1"));
        assert!(manifest.get("workspace").is_some());
    }
    #[test]
    fn real_local_cargo_build_and_graph() {
        let dir = project(&BTreeMap::new()).unwrap();
        cargo(&dir.path().join("Cargo.toml"), &["build", "--offline"]).unwrap();
        let report = graph(
            &cargo(
                &dir.path().join("Cargo.toml"),
                &["metadata", "--format-version", "1", "--offline"],
            )
            .unwrap(),
        )
        .unwrap();
        assert!(report.contains("xcratesx_experiment@0.0.0"));
        assert!(report.contains("dependency edges: 0"));
    }
}
