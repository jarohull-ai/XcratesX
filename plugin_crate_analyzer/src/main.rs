use std::io::{self, Write};
use std::collections::HashSet;
use shared_contract::{JfpMessage, JfpReader};

#[derive(Default)]
struct State {
    last_output: Option<String>,
}

fn analyze_dependencies(roots: &[String], depth: usize) -> String {
    let mut nodes: Vec<String> = roots.to_vec();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();

    for name in roots {
        seen.insert(name.clone());
    }

    for _ in 0..depth {
        let mut next = Vec::new();
        for name in &nodes {
            if seen.contains(name) { continue; }
            seen.insert(name.clone());
            let deps = mock_deps(name);
            for dep in &deps {
                if !seen.contains(dep) {
                    seen.insert(dep.clone());
                    edges.push((name.clone(), dep.clone()));
                    next.push(dep.clone());
                }
            }
        }
        nodes.extend(next);
    }

    let mut report = String::from("=== ANALIZA ZALÉŻNOŚCI ===\n\n");
    report.push_str(&format!("Wierzchołki: {}\n", nodes.len()));
    report.push_str(&format!("Krawędzie: {}\n", edges.len()));
    report.push_str("\nструктура:\n");

    for name in &nodes {
        if name.len() > 25 { break; }
        let neighbors: Vec<String> = edges.iter()
            .filter(|(_, target)| **target == *name)
            .map(|(src, _)| src.clone())
            .collect();
        report.push_str(&format!("  {} <- {}\n", name, neighbors.join(", ")));
    }

    report
}

fn mock_deps(name: &str) -> Vec<String> {
    match name {
        "serde" => vec!["serde_json".to_string(), "bincode".to_string(), "ron".to_string()],
        "serde_json" => vec!["itoa".to_string(), "ryu".to_string(), "serde".to_string()],
        "tokio" => vec!["pin-project".to_string(), "tokio-stream".to_string()],
        "reqwest" => vec!["tokio".to_string(), "http".to_string(), "serde_json".to_string()],
        _ => vec!["serde".to_string(), "log".to_string(), "thiserror".to_string()],
    }
}

fn handle(frame: &JfpMessage, state: &mut State) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let input = frame.body.trim();
            if input.is_empty() {
                return Some(JfpMessage::reply("OK", "cargo_analyzer | wpisz: analyze serde, tokio"));
            }
            if input.starts_with("analyze ") {
                let parts: Vec<&str> = input[9..].split(',').map(|s| s.trim()).collect();
                let roots: Vec<String> = parts.iter().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
                if roots.is_empty() {
                    return Some(JfpMessage::reply("OK", "cargo_analyzer | podaj konfigurację (np. analyze serde, tokio)"));
                }
                let result = analyze_dependencies(&roots, 2);
                state.last_output = Some(result.clone());
                return Some(JfpMessage::reply("OK", result));
            }
            Some(JfpMessage::reply("OK", format!("cargo_analyzer | received: {input}")))
        }
        "TICK" => {
            if let Some(out) = &state.last_output {
                return Some(JfpMessage::reply("OK", out));
            }
            Some(JfpMessage::reply("OK", "cargo_analyzer | gotowe. wpisz: analyze serde, tokio"))
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
        let Some(msg) = handle(&frame, &mut state) else { continue; };
        if writeln!(stdout, "{}", msg.to_jfp_string()).is_err() { break; }
        if stdout.flush().is_err() { break; }
    }
}
