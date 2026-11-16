use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use shared_contract::{JfpMessage, JfpReader};
use tokio::sync::{mpsc, Mutex};

const PER_PAGE: u8 = 10;
const USER_AGENT: &str = "XcratesX/0.1 (github.com/jarohull-ai/XcratesX)";

enum ScoutState {
    Idle,
    Searching,
    Results(String),
    Detail(String),
    Error(String),
}

struct State {
    query: String,
    scout: ScoutState,
    sort: SortMode,
}

#[derive(Clone, Copy)]
enum SortMode {
    Relevance,
    Downloads,
    Recent,
    New,
}

impl SortMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Recent => "recent-downloads",
            Self::New => "recent-updates",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Relevance => "trafnosc",
            Self::Downloads => "pobrania",
            Self::Recent => "ostatnie pobrania",
            Self::New => "ostatnie aktualizacje",
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            query: String::new(),
            scout: ScoutState::Idle,
            sort: SortMode::Downloads,
        }
    }
}

#[derive(Deserialize)]
struct ApiResponse {
    crates: Vec<CrateEntry>,
    meta: Meta,
}

#[derive(Deserialize)]
struct Meta {
    total: u64,
}

#[derive(Deserialize)]
struct CrateEntry {
    name: String,
    #[serde(default)]
    description: Option<String>,
    max_stable_version: Option<String>,
    downloads: u64,
    recent_downloads: Option<u64>,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    documentation: Option<String>,
    updated_at: String,
}

#[derive(Deserialize)]
struct CrateDetailResponse {
    #[serde(rename = "crate")]
    krate: CrateEntry,
    #[serde(default)]
    versions: Vec<VersionEntry>,
    #[serde(default)]
    keywords: Vec<KeywordEntry>,
    #[serde(default)]
    categories: Vec<CategoryEntry>,
}

#[derive(Deserialize)]
struct VersionEntry {
    num: String,
    #[serde(default)]
    yanked: bool,
    downloads: u64,
}

#[derive(Deserialize)]
struct KeywordEntry {
    keyword: String,
}

#[derive(Deserialize)]
struct CategoryEntry {
    category: String,
}

fn send_reply(stdout: &mut io::Stdout, message: JfpMessage) -> bool {
    if writeln!(stdout, "{}", message.to_jfp_string()).is_err() {
        return false;
    }
    stdout.flush().is_ok()
}

fn format_downloads(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_search_results(response: &ApiResponse, query: &str, sort: SortMode) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "CRATE SCOUT | Wyniki dla \"{query}\" (sort: {}, znaleziono: {})\n",
        sort.label(),
        response.meta.total
    );

    if response.crates.is_empty() {
        out.push_str("  (brak wynikow)\n");
        return out;
    }

    for (i, c) in response.crates.iter().enumerate() {
        let desc = c
            .description
            .as_deref()
            .unwrap_or("-")
            .chars()
            .take(80)
            .collect::<String>();

        let version = c.max_stable_version.as_deref().unwrap_or("?");
        let recent = c.recent_downloads.map(format_downloads).unwrap_or_default();

        let _ = writeln!(
            out,
            "{:>2}. {} v{} | dl: {} (recent: {}) | {}",
            i + 1,
            c.name,
            version,
            format_downloads(c.downloads),
            recent,
            desc,
        );
    }

    let _ = writeln!(
        out,
        "\nWpisz nazwe crate'a aby zobaczyc szczegoly (np. \"info hound\")"
    );
    let _ = writeln!(
        out,
        "Sortowanie: \"sort dl\" | \"sort rel\" | \"sort recent\" | \"sort new\""
    );

    out
}

fn format_detail(resp: &CrateDetailResponse) -> String {
    let c = &resp.krate;
    let mut out = String::new();

    let _ = writeln!(out, "CRATE SCOUT | Szczegoly: {}\n", c.name);

    let desc = c.description.as_deref().unwrap_or("-");
    let _ = writeln!(out, "  Opis:       {desc}");
    let _ = writeln!(
        out,
        "  Wersja:     {}",
        c.max_stable_version.as_deref().unwrap_or("?")
    );
    let _ = writeln!(
        out,
        "  Pobrania:   {} (total)",
        format_downloads(c.downloads)
    );

    if let Some(recent) = c.recent_downloads {
        let _ = writeln!(out, "  Ostatnie:   {} (90 dni)", format_downloads(recent));
    }

    let updated = c.updated_at.get(..10).unwrap_or(&c.updated_at);
    let _ = writeln!(out, "  Aktualizacja: {updated}");

    if let Some(repo) = &c.repository {
        let _ = writeln!(out, "  Repo:       {repo}");
    }

    if let Some(docs) = &c.documentation {
        let _ = writeln!(out, "  Docs:       {docs}");
    }

    if !resp.keywords.is_empty() {
        let kw: Vec<&str> = resp.keywords.iter().map(|k| k.keyword.as_str()).collect();
        let _ = writeln!(out, "  Tagi:       {}", kw.join(", "));
    }

    if !resp.categories.is_empty() {
        let cats: Vec<&str> = resp
            .categories
            .iter()
            .map(|c| c.category.as_str())
            .collect();
        let _ = writeln!(out, "  Kategorie:  {}", cats.join(", "));
    }

    if !resp.versions.is_empty() {
        let _ = writeln!(out, "\n  --- WERSJE (ostatnie 8) ---");
        for v in resp.versions.iter().take(8) {
            let yanked = if v.yanked { " [YANKED]" } else { "" };
            let _ = writeln!(
                out,
                "  v{:<12} dl: {}{}",
                v.num,
                format_downloads(v.downloads),
                yanked
            );
        }
    }

    let _ = writeln!(
        out,
        "\n  Cargo.toml: {} = \"{}\"",
        c.name,
        c.max_stable_version.as_deref().unwrap_or("*")
    );

    out
}

async fn search_crates(query: String, sort: SortMode, state: Arc<Mutex<State>>) {
    let url = format!(
        "https://crates.io/api/v1/crates?q={}&per_page={PER_PAGE}&sort={}",
        urlencoded(&query),
        sort.as_str()
    );

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    let result = client.get(&url).send().await;

    let mut guard = state.lock().await;
    if guard.query != query {
        return;
    }

    match result {
        Ok(resp) => match resp.json::<ApiResponse>().await {
            Ok(data) => {
                let report = format_search_results(&data, &query, sort);
                guard.scout = ScoutState::Results(report);
            }
            Err(err) => {
                guard.scout = ScoutState::Error(format!("Blad parsowania: {err}"));
            }
        },
        Err(err) => {
            guard.scout = ScoutState::Error(format!("Blad HTTP: {err}"));
        }
    }
}

async fn fetch_detail(name: String, state: Arc<Mutex<State>>) {
    let url = format!("https://crates.io/api/v1/crates/{name}");

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    let result = client.get(&url).send().await;

    let mut guard = state.lock().await;

    match result {
        Ok(resp) => match resp.json::<CrateDetailResponse>().await {
            Ok(data) => {
                let report = format_detail(&data);
                guard.scout = ScoutState::Detail(report);
            }
            Err(err) => {
                guard.scout = ScoutState::Error(format!("Blad parsowania: {err}"));
            }
        },
        Err(err) => {
            guard.scout = ScoutState::Error(format!("Blad HTTP: {err}"));
        }
    }
}

fn urlencoded(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

fn handle_command(input: &str, state: &mut State) -> (JfpMessage, Command) {
    let input = input.trim();

    if input.is_empty() {
        return (
            JfpMessage::reply("OK", "SCOUT | Puste zapytanie."),
            Command::None,
        );
    }

    if let Some(name) = input
        .strip_prefix("info ")
        .or_else(|| input.strip_prefix("INFO "))
    {
        let name = name.trim().to_owned();
        return (
            JfpMessage::reply("OK", format!("SCOUT | Pobieram szczegoly: {name}")),
            Command::Detail(name),
        );
    }

    if let Some(mode) = input
        .strip_prefix("sort ")
        .or_else(|| input.strip_prefix("SORT "))
    {
        let mode = mode.trim().to_lowercase();
        state.sort = match mode.as_str() {
            "dl" | "downloads" => SortMode::Downloads,
            "rel" | "relevance" => SortMode::Relevance,
            "recent" => SortMode::Recent,
            "new" => SortMode::New,
            _ => {
                return (
                    JfpMessage::reply(
                        "OK",
                        "SCOUT | Nieznane sortowanie. Uzyj: dl, rel, recent, new",
                    ),
                    Command::None,
                );
            }
        };

        if state.query.is_empty() {
            return (
                JfpMessage::reply(
                    "OK",
                    format!(
                        "SCOUT | Sortowanie: {}. Wpisz zapytanie.",
                        state.sort.label()
                    ),
                ),
                Command::None,
            );
        }

        state.scout = ScoutState::Searching;
        return (
            JfpMessage::reply(
                "OK",
                format!(
                    "SCOUT | Szukam \"{}\" (sort: {})...",
                    state.query,
                    state.sort.label()
                ),
            ),
            Command::Search(state.query.clone()),
        );
    }

    state.query = input.to_owned();
    state.scout = ScoutState::Searching;

    (
        JfpMessage::reply(
            "OK",
            format!(
                "SCOUT | Szukam \"{}\" (sort: {})...",
                state.query,
                state.sort.label()
            ),
        ),
        Command::Search(state.query.clone()),
    )
}

enum Command {
    None,
    Search(String),
    Detail(String),
}

fn handle_tick(state: &State) -> JfpMessage {
    match &state.scout {
        ScoutState::Idle => JfpMessage::reply(
            "OK",
            "CRATE SCOUT | Wyszukiwarka crates.io\n\
             Wpisz fraze (np. \"audio wav\", \"http client async\")\n\
             Komendy: \"info <nazwa>\" | \"sort dl/rel/recent/new\"",
        ),
        ScoutState::Searching => {
            JfpMessage::reply("WAIT", format!("SCOUT | Szukam \"{}\"...", state.query))
        }
        ScoutState::Results(data) | ScoutState::Detail(data) => {
            JfpMessage::reply("OK", data.as_str())
        }
        ScoutState::Error(err) => JfpMessage::reply("ERROR", format!("SCOUT | {err}")),
    }
}

#[tokio::main]
async fn main() {
    let state = Arc::new(Mutex::new(State::default()));

    let (tx, mut rx) = mpsc::unbounded_channel::<JfpMessage>();

    tokio::task::spawn_blocking(move || {
        let stdin = io::stdin();
        let mut reader = JfpReader::new(stdin.lock());

        while let Some(frame) = reader.next_frame() {
            if tx.send(frame).is_err() {
                break;
            }
        }
    });

    let mut stdout = io::stdout();

    while let Some(frame) = rx.recv().await {
        match frame.intent.as_deref() {
            Some("COMMAND") => {
                let (reply, cmd) = {
                    let mut guard = state.lock().await;
                    handle_command(&frame.body, &mut guard)
                };

                if !send_reply(&mut stdout, reply) {
                    break;
                }

                match cmd {
                    Command::Search(query) => {
                        let sort = state.lock().await.sort;
                        let sc = Arc::clone(&state);
                        tokio::spawn(search_crates(query, sort, sc));
                    }
                    Command::Detail(name) => {
                        let sc = Arc::clone(&state);
                        tokio::spawn(fetch_detail(name, sc));
                    }
                    Command::None => {}
                }
            }
            Some("TICK") => {
                let reply = {
                    let guard = state.lock().await;
                    handle_tick(&guard)
                };

                if !send_reply(&mut stdout, reply) {
                    break;
                }
            }
            _ => {}
        }
    }
}
