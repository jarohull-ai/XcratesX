# XcratesX

Silnik TUI (ratatui/crossterm) sterujący zestawem wtyczek domenowych przez protokół **JFP v5.4.0** po stdio. Każda wtyczka to osobny proces — izolacja awarii, hot-reload, presety domenowe.

## Architektura

```
                    ┌─────────────┐
                    │ presets.toml │  ← presety domenowe
                    └──────┬──────┘
                           │
┌──────────────────────────▼───────────────────────────┐
│                    host_engine                         │
│  TUI (ratatui) + preset selector + JFP dispatcher     │
└───┬──────┬──────┬──────┬──────┬──────┬──────┬────────┘
    │      │      │      │      │      │      │
  stdio  stdio  stdio  stdio  stdio  stdio  stdio
    │      │      │      │      │      │      │
    ▼      ▼      ▼      ▼      ▼      ▼      ▼
 OSINT  SEARCH  AUDIO SYSINFO SCOUT  MEDIA CRUCIBLE
```

- **jfp-contract** (`shared_contract/`) — biblioteka JFP: `JfpMessage`, `JfpReader` — [crates.io](https://crates.io/crates/jfp-contract)
- **host_engine** — TUI z ekranem powitalnym (presety), spawn procesów, TICK/COMMAND
- **workspace.toml** — rejestr wtyczek (sloty, klucze, opisy)
- **presets.toml** — presety domenowe (DevOps, Audio, Recon, Wszystko)
- **forge.sh** — generator nowych wtyczek z szablonu

## Wtyczki

| Slot | Nazwa | Crate | Opis |
|------|-------|-------|------|
| 1 | OSINT | reqwest | Skanowanie IP/domen (ip-api.com) |
| 2 | SEARCH | scraper | Web scraper — struktura HTML |
| 3 | AUDIO | hound | Analiza plików WAV (peak, RMS, crest) |
| 4 | SYSINFO | sysinfo | Monitor systemu (CPU, RAM, dyski, sieć, procesy) |
| 5 | SCOUT | reqwest | Wyszukiwarka crates.io (search, info, sort) |
| 6 | MEDIA | lofty | Metadane audio (MP3, FLAC, WAV, OGG — tagi, bitrate, okładki) |
| 9 | CRUCIBLE | — | Stress-test: celowy crash po 6 tickach |

## Presety

Przy starcie wybierasz domenę — ładują się odpowiednie wtyczki:

- **DevOps** — OSINT + SCOUT + SYSINFO
- **Audio** — AUDIO + MEDIA
- **Recon** — OSINT + SEARCH + SCOUT
- **Wszystko** — wszystkie zainstalowane

## Protokół JFP v5.4.0

```
[JFP_HEADER]
VERSION: v5.4.0
STATUS: OK          ← odpowiedź (plugin → host)
INTENT: COMMAND     ← żądanie (host → plugin)
[END_HEADER]
[MESSAGE_BODY]
treść
[END_MESSAGE]
```

## Uruchomienie

```sh
cargo build
./target/debug/host_engine
```

## Klawisze

- `↑↓` / `1-9` — nawigacja / przełączanie wtyczek
- `Tab` — tryb komend
- `Enter` — wyślij komendę
- `p` — powrót do presetów
- `r` — przeładuj workspace.toml
- `q` / `Esc` — wyjście

## Nowa wtyczka

```sh
./forge.sh <nazwa> [crate1 crate2 ...]
# Przykład:
./forge.sh recon reqwest scraper
```
