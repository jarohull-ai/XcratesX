use std::io::{self, Write};

use shared_contract::{JfpMessage, JfpReader};

#[derive(Default)]
struct State {
    last_output: Option<String>,
}

fn suggest_crates(query: &str) -> String {
    let crates = [
        ("serde", "Serialize/Deserialize"),
        ("serde_json", "JSON serialization"),
        ("tokio", "Async runtime"),
        ("reqwest", "HTTP client"),
        ("surf", "Simple HTTP"),
        ("url", "URL parsing"),
        ("scraper", "HTML parsing"),
        ("regex", "Regex matching"),
        ("clap", "CLI arguments"),
        ("config", "Config loading"),
        ("envy", "Env config"),
        ("log", "Logging"),
        ("tracing", "Structured logging"),
        ("anyhow", "Error handling"),
        ("thiserror", "Custom errors"),
        ("rand", "Random data"),
        ("chrono", "Time/date"),
        ("uuid", "UUID generation"),
        ("base64", "Base64 encoding"),
        ("hex", "Hex encoding"),
        ("sha2", "Hashing"),
        ("ring", "Crypto"),
        ("openssl", "TLS"),
        ("bincode", "Binary serialization"),
        ("cbor", "CBOR format"),
        ("ron", "Rusty Object Notation"),
        ("toml", "TOML config"),
        ("yaml", "YAML config"),
        ("lazy_static", "Global statics"),
        ("once_cell", "Lazy statics"),
        ("spin", "Spinlocks"),
        ("crossbeam", "Concurrency"),
        ("rayon", "Parallel iterators"),
        ("threadpool", "Thread pool"),
        ("num_cpus", "CPU count"),
        ("signal-hook", "Signal handling"),
        ("indicatif", "Progress bars"),
        ("console", "Console output"),
        ("colored", "Colored output"),
        ("human_bytes", "Human readable sizes"),
        ("dirs", "User dirs"),
        ("dirs_next", "User dirs v2"),
        ("home", "Home directory"),
        ("tempfile", "Temp files"),
        ("flate2", "Compression"),
        ("zstd", "Zstd compression"),
        ("lz4", "LZ4 compression"),
        ("bzip2", "Bzip2"),
        ("xz2", "XZ compression"),
        ("tar", "TAR archives"),
        ("zip", "ZIP archives"),
        ("curl", "cURL bindings"),
        ("hyper", "HTTP/2 server"),
        ("actix-web", "Web framework"),
        ("axum", "Modern web framework"),
        ("warp", "Web framework"),
        ("rocket", "Web framework"),
        ("iron", "Web framework"),
        ("gotham", "Web framework"),
        ("tide", "Small web framework"),
        ("rouge", "Web framework"),
        ("poem", "Async web framework"),
        ("tiny_http", "Simple HTTP server"),
        ("miniserde", "Minimal serde"),
        ("serde_bytes", "Optimized bytes"),
        ("indexmap", "Ordered map"),
        ("hashbrown", "Hash map"),
        ("aho-corasick", "String searching"),
        ("memchr", "Byte searching"),
        ("regex-syntax", "Regex syntax"),
        ("regex-automata", "Regex automata"),
        ("lazycell", "Lazy initialization"),
        ("slotmap", "Slot maps"),
        ("flume", "Channels"),
        ("crossbeam-channel", "Channels"),
        ("futures", "Async futures"),
        ("async-stream", "Async streams"),
        ("pin-project", "Pin projection"),
        ("pin-utils", "Pin utilities"),
        ("async-trait", "Async traits"),
        ("tower", "Service traits"),
        ("tower-http", "HTTP service"),
        ("tower-service", "Service trait"),
        ("interpreter", "Evaluation"),
        ("petgraph", "Graphs"),
        ("heapless", "No-alloc"),
        ("typenum", "Type-level numbers"),
        ("const-generic", "Const generics"),
        ("generic-array", "Generic arrays"),
        ("bytemuck", "Zero-copy"),
        ("alignor", "Alignment"),
        ("integral", "Integral traits"),
        ("fixed", "Fixed point"),
        ("bigdecimal", "Big decimals"),
        ("num-bigint", "Big integers"),
        ("num-rational", "Rational numbers"),
        ("num-complex", "Complex numbers"),
        ("rust_decimal", "Decimal"),
        ("rkyv", "Zero-copy serialization"),
        ("zerocopy", "Zero-copy"),
        ("slice-group-by", "Grouping"),
        ("itertools", "Iterator tools"),
        ("either", "Either type"),
        ("smallvec", "Small vectors"),
        ("arrayvec", "Array vectors"),
        ("linked_hash_map", "Linked hash map"),
        ("indexmap", "Ordered map"),
        ("fxhash", "FX hash"),
        ("fnv", "FNV hash"),
        ("ahash", "AHash"),
        ("twox-hash", "Twox hash"),
        ("farmhash", "FarmHash"),
        ("wyhash", "WyHash"),
    ];

    crates
        .iter()
        .filter(|(name, description)| {
            name.contains(query)
                || description.to_lowercase().contains(&query.to_lowercase())
                || query.is_empty()
        })
        .take(10)
        .map(|(n, d)| format!("  • {n} - {d}"))
        .collect()
}

fn handle(frame: &JfpMessage, state: &mut State) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            let input = frame.body.trim();
            if input.is_empty() {
                return Some(JfpMessage::reply("OK", "Search the catalog: suggest serde"));
            }
            if let Some(query) = input.strip_prefix("suggest ") {
                let q = query.trim();
                let result = suggest_crates(q);
                state.last_output = Some(result.clone());
                return Some(JfpMessage::reply("OK", result));
            }
            if input.is_empty() {
                return Some(JfpMessage::reply("OK", "CATALOG | Use: suggest <query>"));
            }
            Some(JfpMessage::reply(
                "OK",
                format!("crate_kit | received: {input}"),
            ))
        }
        "TICK" => {
            if let Some(out) = &state.last_output {
                return Some(JfpMessage::reply("OK", out));
            }
            Some(JfpMessage::reply(
                "OK",
                "CATALOG | Use: suggest <query>; :add NAME:VERSION saves a selection",
            ))
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
        let Some(msg) = handle(&frame, &mut state) else {
            continue;
        };
        if writeln!(stdout, "{}", msg.to_jfp_string()).is_err() {
            break;
        }
        if stdout.flush().is_err() {
            break;
        }
    }
}
