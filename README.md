# XcratesX: Meccano/Lego dla crate'ów

**XcratesX** to uniwersalna platforma do składania, testowania i wdrażania
konfiguracji crate'ów z **crates.io** i **GitHub**. Każda wtyczka to
niezależny "układanka" — pełni ogólną funkcję, wymaga lekkiego
narzędzia do łączenia. Zielony kod na pomostach.

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

## Komponenty

| Komponent | Kategoria | Opis |
|-----------|-----------|------|
| **plugin_crate_assembler** | *Mekkański ładowač* | Składa konfiguracje crate'ów, testuje buildy, raportuje wyniki |
| **plugin_crate_scout** | *Szkolniak* | Wyszukiwarka crates z crates.io (REST API) |
| **plugin_sysinfo** | *Miernik* | Telemetria systemu (CPU, RAM, dyski, sieć) |
| **plugin_net_diag** | *Detektor* | Diagnostyka połączeń (Viper ↔ Venom) |
| **plugin_osint** | *Skaner* | Skanowanie IP/domen |
| **plugin_search** | *Fensteroman* | Web scraper — struktura HTML |
| **plugin_audio** | *Tonografia* | Analiza plików WAV |
| **plugin_media_meta** | *Tajnik* | Metadane audio (tagi, bitrate) |
| **plugin_crucible** | *Próbnik* | Stress-test — celowy crash po 6 tickach |

## Presety

- **DevOps** — net, mon, scout, sys
- **Meccano** — *assemblek + scout + sysinfo*
- **Cluster** — net, sys, osint
- **Audio** — audio + media
- **Recon** — osint + search + scout
- **Wszystko** — wszystkie wtyczki

## Krakerska notatka

Zasada **interchangeability**: każda wtyczka zgadza się na `JFP v5.4.0`
przez stdio. Dodałeś chwilę, a Twój plugin pojawi się w presecie.

```
╔══════════════════════════════════════════════════════╗
║  🦀 XcratesX 0.1.0 — Meccano/Lego for Crates       ║
║  Składaj, testuj, dystrybuuj — jak w LEGO              ║
╚══════════════════════════════════════════════════════╝
```

## Nowa wtyczka

```sh
./forge.sh <nazwa> [crate1 crate2 ...]
# Przykład: ./forge.sh recon reqwest scraper
```

## Protokół JFP v5.4.0

```
[JFP_HEADER]
VERSION: v5.4.0
STATUS: OK
INTENT: COMMAND
[END_HEADER]
[MESSAGE_BODY]
treść
[END_MESSAGE]
```

