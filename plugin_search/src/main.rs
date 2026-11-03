use std::io::{self, Write};
use std::sync::Arc;

use scraper::{Html, Selector};
use shared_contract::{JfpMessage, JfpReader};
use tokio::sync::{mpsc, Mutex};

enum ScrapeState {
    Idle,
    Fetching,
    Done(String),
    Error(String),
}

struct SearchState {
    url: String,
    scrape: ScrapeState,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            url: String::new(),
            scrape: ScrapeState::Idle,
        }
    }
}

fn send_reply(stdout: &mut io::Stdout, message: JfpMessage) -> bool {
    if writeln!(stdout, "{}", message.to_jfp_string()).is_err() {
        return false;
    }
    stdout.flush().is_ok()
}

async fn scrape_url(url: String, state: Arc<Mutex<SearchState>>) {
    let result = reqwest::get(&url).await;

    let html = match result {
        Ok(resp) => match resp.text().await {
            Ok(body) => body,
            Err(err) => {
                let mut guard = state.lock().await;
                if guard.url == url {
                    guard.scrape = ScrapeState::Error(format!("Blad odczytu body: {err}"));
                }
                return;
            }
        },
        Err(err) => {
            let mut guard = state.lock().await;
            if guard.url == url {
                guard.scrape = ScrapeState::Error(format!("Blad HTTP: {err}"));
            }
            return;
        }
    };

    let report = extract_structure(&html);

    let mut guard = state.lock().await;
    if guard.url == url {
        guard.scrape = ScrapeState::Done(report);
    }
}

fn extract_structure(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut report = String::new();

    if let Ok(sel) = Selector::parse("title") {
        if let Some(el) = document.select(&sel).next() {
            let title: String = el.text().collect();
            report.push_str(&format!("TYTUL: {}\n", title.trim()));
        }
    }

    if let Ok(sel) = Selector::parse("meta[name=description]") {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                report.push_str(&format!("OPIS: {}\n", content.trim()));
            }
        }
    }

    report.push_str("\n--- NAGLOWKI ---\n");
    for tag in ["h1", "h2", "h3"] {
        if let Ok(sel) = Selector::parse(tag) {
            for el in document.select(&sel).take(10) {
                let text: String = el.text().collect();
                let text = text.trim();
                if !text.is_empty() {
                    report.push_str(&format!("[{tag}] {text}\n"));
                }
            }
        }
    }

    let mut links = Vec::new();
    if let Ok(sel) = Selector::parse("a[href]") {
        for el in document.select(&sel).take(30) {
            if let Some(href) = el.value().attr("href") {
                let label: String = el.text().collect();
                let label = label.trim().chars().take(60).collect::<String>();
                if !href.is_empty() && !href.starts_with('#') && !href.starts_with("javascript:") {
                    links.push((href.to_owned(), label));
                }
            }
        }
    }

    if !links.is_empty() {
        report.push_str(&format!("\n--- LINKI ({}) ---\n", links.len()));
        for (href, label) in &links {
            if label.is_empty() {
                report.push_str(&format!("  {href}\n"));
            } else {
                report.push_str(&format!("  {label} -> {href}\n"));
            }
        }
    }

    if report.is_empty() {
        report.push_str("(brak rozpoznanej struktury HTML)");
    }

    report
}

fn handle_command(input: &str, state: &mut SearchState) -> JfpMessage {
    let url = input.trim();

    if url.is_empty() {
        return JfpMessage::reply("OK", "SEARCH | Pusty URL — podaj adres strony.");
    }

    let url = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("https://{url}")
    } else {
        url.to_owned()
    };

    state.url = url.clone();
    state.scrape = ScrapeState::Fetching;

    JfpMessage::reply("OK", format!("SEARCH | Pobieram: {url}"))
}

fn handle_tick(state: &SearchState) -> JfpMessage {
    match &state.scrape {
        ScrapeState::Idle => JfpMessage::reply(
            "OK",
            "SEARCH | Gotowy. Podaj URL w wierszu polecen (np. crates.io)",
        ),
        ScrapeState::Fetching => JfpMessage::reply(
            "WAIT",
            format!("SEARCH | Pobieranie: {} ...", state.url),
        ),
        ScrapeState::Done(data) => JfpMessage::reply(
            "OK",
            format!("SEARCH | Wynik dla {}:\n{data}", state.url),
        ),
        ScrapeState::Error(err) => JfpMessage::reply(
            "ERROR",
            format!("SEARCH | Blad dla {}: {err}", state.url),
        ),
    }
}

#[tokio::main]
async fn main() {
    let state = Arc::new(Mutex::new(SearchState::default()));

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
                let reply = {
                    let mut guard = state.lock().await;
                    handle_command(&frame.body, &mut guard)
                };

                if !send_reply(&mut stdout, reply) {
                    break;
                }

                let url = state.lock().await.url.clone();
                if !url.is_empty() {
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(scrape_url(url, state_clone));
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
