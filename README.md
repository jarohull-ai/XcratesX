# 🦀 XcratesX — Meccano/Lego dla crate'ów

## Co to jest?

**XcratesX** to uniwersalna platforma do **składania, testowania i wdrażania konfiguracji crate'ów** z **crates.io** i **GitHub**. Każda wtyczka to niezależny "układanka" — możesz ją podłączyć do systemu, przetestować i użyć jak klocek z LEGO.

Projekt powstał z myślą o osobach, które chcą:
- **eksperymentować z różnymi crate'ami** bez zakłócania głównego projektu,
- **testować konfiguracje** przed wdrożeniem na produkcję,
- **tworzyć własne preset-y** pod swoje potrzeby,
- **dostosować narzędzie do siebie** — każdy użytkownik może dodać własną wtyczkę.

## Dlaczego powstał?

W ekosystemie Rust brakuje elastycznego narzędzia do **eksperymentowania z crate'ami** w sposób modularny i izolowany. Zazwyczaj:
- dodawanie crate'ów do Cargo.toml wymaga przebudowania całego projektu,
- testowanie kombinacji crate'ów jest czasochłonne,
- nie ma prostego sposobu na "wymienialne moduły" funkcjonalności.

**XcratesX** rozwiązuje ten problem: każda wtyczka działa jako osobny proces, komunikuje się przez protokół **JFP v5.4.0** po stdio, a Ty wybierasz, które wtyczki chcesz uruchomić jednocześnie.

## Co to daje?

| Korzyść | Opis |
|---------|------|
| 🧩 **Modularność** | Każda wtyczka to osobny proces — awaria jednej nie psuje reszty |
| 🔄 **Hot-reload** | Wciśnij `r` w TUI, aby przeładować konfigurację bez restartu |
| 🎯 **Presety domenowe** | Gotowe zestawy wtyczek do DevOps, Audio, Recon, Meccano |
| 🔧 **Własne wtyczki** | `./forge.sh <nazwa>` tworzy nową wtyczkę z szablonu |
| 📦 **Składanie crate'ów** | `plugin_crate_assembler` testuje i raportuje konfiguracje crate'ów |
| 🔍 **Analiza zależności** | `plugin_crate_analyzer` rysuje graf zależności crate'ów |
| 📚 **Katalog crate'ów** | `plugin_crate_kit` sugeruje crate'y na podstawie zapytania |

## Jak obsługiwać?

### Uruchomienie

```bash
cd /home/jaro/Work/XcratesX_test
cargo build --release
./target/release/host_engine
```

### Skróty klawiszowe (w TUI)

| Klawisz | Akcja |
|---------|-------|
| `↑` / `↓` | Nawigacja po presetach i wtyczkach |
| `1`-`9` | Szybki wybór wtyczki po numerze |
| `Enter` | Wybierz preset / wyślij komendę |
| `Tab` | Tryb komend (wpisz komendę do aktywnej wtyczki) |
| `p` | Powrót do ekranu wyboru presetów |
| `r` | Przeładuj `workspace.toml` (hot-reload) |
| `q` / `Esc` | Wyjście z aplikacji |

### Przykładowy workflow (preset Meccano)

1. Uruchom `./target/release/host_engine`
2. Wybierz preset **Meccano** (slot 4)
3. Przełącz się na wtyczkę `crate_assembler` (naciśnij `4`)
4. W trybie komend (`Tab`) wpisz:
   ```
   test:serde:1, serde_json:1
   ```
5. Otrzymasz raport: `BUILD OK: serde 1`, `BUILD OK: serde_json 1`
6. Przełącz się na `crate_analyzer` i wpisz:
   ```
   analyze serde, tokio
   ```
7. Otrzymasz graf zależności crate'ów

## Komponenty

| Wtyczka | Kategoria | Co robi |
|---------|-----------|---------|
| `plugin_crate_assembler` | 🏗️ Składanie | Testuje konfiguracje crate'ów, raportuje wyniki buildów |
| `plugin_crate_kit` | 📦 Katalog | Sugeruje crate'y na podstawie zapytania (np. `suggest serde`) |
| `plugin_crate_analyzer` | 🔍 Analiza | Rysuje graf zależności crate'ów (`analyze serde, tokio`) |
| `plugin_crate_scout` | 🔍 Wyszukiwarka | Przeszukuje crates.io przez REST API |
| `plugin_sysinfo` | 📊 Telemetria | Monitoruje CPU, RAM, dyski, procesy |
| `plugin_net_diag` | 🌐 Sieć | Diagnostyka połączeń (Viper ↔ Venom) |
| `plugin_osint` | 🌍 OSINT | Skanowanie IP/domen przez ip-api.com |
| `plugin_search` | 🕷️ Scraper | Web scraper — struktura HTML |
| `plugin_audio` | 🎵 Audio | Analiza plików WAV (peak, RMS, crest) |
| `plugin_media_meta` | 🎚️ Media | Metadane audio (tagi, bitrate) |
| `plugin_crucible` | ⚡ Test | Stress-test — celowy crash po 6 tickach |

## Presety (gotowe zestawy wtyczek)

| Preset | Do czego służy | Wtyczki |
|--------|---------------|---------|
| **DevOps** | Monitoring i skanowanie | osint, scout, sysinfo, net_diag |
| **Meccano** | Składanie i analiza crate'ów | crate_assembler, crate_kit, crate_analyzer, crate_scout |
| **Cluster** | Diagnostyka klastra | net_diag, sysinfo, osint |
| **Audio** | Analiza dźwięku | audio, media_meta |
| **Recon** | OSINT i przeszukiwanie | osint, search, crate_scout |
| **Wszystko** | Wszystko naraz | wszystkie zainstalowane wtyczki |

## Protokół JFP v5.4.0

Wszystkie wtyczki komunikują się przez standard **JFP v5.4.0** po stdio:

```
[JFP_HEADER]
VERSION: v5.4.0
STATUS: OK          ← odpowiedź (plugin → host)
INTENT: COMMAND     ← żądanie (host → plugin)
[END_HEADER]
[MESSAGE_BODY]
treść komendy
[END_MESSAGE]
```

## Nowa wtyczka (forge.sh)

Tworzenie własnej wtyczki w jednej komendzie:

```bash
./forge.sh <nazwa> [crate1 crate2 ...]

# Przykład:
./forge.sh recon reqwest scraper
```

To skopiuje szablon, doda zależności, zarejestruje w `workspace.toml` i zbuduje wtyczkę.

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
 CrateAss  SCOUT  SYSINFO CLUSTER OSINT  SEARCH MEDIA
```

## Licencja

MIT — zobacz plik `LICENSE`.

---

*Budowane dla Ciebie — dostosuj do swoich potrzeb, rozwijaj, dziel się!*
