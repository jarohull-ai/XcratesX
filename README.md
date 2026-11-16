# 🦀 XcratesX — Meccano/Lego for Crates

## What is it?

**XcratesX** is a universal platform for **assembling, testing, and deploying crate configurations** from **crates.io** and **GitHub**. Each plugin is an independent "building block" — you can plug it in, test it, and use it like a LEGO brick.

The project was created for people who want to:
- **experiment with different crates** without breaking their main project,
- **test configurations** before deploying to production,
- **create custom presets** for their own needs,
- **customize the tool** — every user can add their own plugin.

## Why was it created?

The Rust ecosystem lacks a flexible tool for **modular and isolated experimentation with crates**. Usually:
- adding crates to Cargo.toml requires rebuilding the whole project,
- testing crate combinations is time-consuming,
- there's no simple way to have "swappable modules" of functionality.

**XcratesX** solves this problem: each plugin runs as a separate process, communicates via the **JFP v5.4.0** protocol over stdio, and you choose which plugins to run at the same time.

## What does it give you?

| Benefit | Description |
|---------|-------------|
| 🧩 **Modularity** | Each plugin is a separate process — failure of one doesn't break the rest |
| 🔄 **Hot-reload** | Press `r` in TUI to reload configuration without restarting |
| 🎯 **Domain presets** | Ready-made plugin sets for DevOps, Audio, Recon, Meccano |
| 🔧 **Your own plugins** | `./forge.sh <name>` creates a new plugin from template |
| 📦 **Crate assembly** | `plugin_crate_assembler` tests and reports crate configurations |
| 🔍 **Dependency analysis** | `plugin_crate_analyzer` draws crate dependency graphs |
| 📚 **Crate catalog** | `plugin_crate_kit` suggests crates based on query |

## How to use it?

### Running

```bash
cd /home/jaro/Work/XcratesX_test
cargo build --release
./target/release/host_engine
```

### Keyboard shortcuts (in TUI)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate presets and plugins |
| `1`-`9` | Quick plugin selection by number |
| `Enter` | Select preset / send command |
| `Tab` | Command mode (type command to active plugin) |
| `p` | Return to preset selection screen |
| `r` | Reload `workspace.toml` (hot-reload) |
| `q` / `Esc` | Quit application |

### Example workflow (Meccano preset)

1. Run `./target/release/host_engine`
2. Select preset **Meccano** (slot 4)
3. Switch to `crate_assembler` plugin (press `4`)
4. In command mode (`Tab`) type:
   ```
   test:serde:1, serde_json:1
   ```
5. You get report: `BUILD OK: serde 1`, `BUILD OK: serde_json 1`
6. Switch to `crate_analyzer` plugin and type:
   ```
   analyze serde, tokio
   ```
7. You get crate dependency graph

## Components

| Plugin | Category | What it does |
|--------|----------|--------------|
| `plugin_crate_assembler` | 🏗️ Assembly | Tests crate configurations, reports build results |
| `plugin_crate_kit` | 📦 Catalog | Suggests crates based on query (e.g. `suggest serde`) |
| `plugin_crate_analyzer` | 🔍 Analysis | Draws crate dependency graph (`analyze serde, tokio`) |
| `plugin_crate_scout` | 🔍 Search | Searches crates.io via REST API |
| `plugin_sysinfo` | 📊 Telemetry | Monitors CPU, RAM, disks, processes |
| `plugin_net_diag` | 🌐 Network | Connection diagnostics (Viper ↔ Venom) |
| `plugin_osint` | 🌍 OSINT | IP/domain scanning via ip-api.com |
| `plugin_search` | 🕷️ Scraper | Web scraper — HTML structure |
| `plugin_audio` | 🎵 Audio | WAV file analysis (peak, RMS, crest) |
| `plugin_media_meta` | 🎚️ Media | Audio metadata (tags, bitrate) |
| `plugin_crucible` | ⚡ Test | Stress-test — intentional crash after 6 ticks |

## Presets (ready plugin sets)

| Preset | Purpose | Plugins |
|--------|---------|---------|
| **DevOps** | Monitoring and scanning | osint, scout, sysinfo, net_diag |
| **Meccano** | Crate assembly and analysis | crate_assembler, crate_kit, crate_analyzer, crate_scout |
| **Cluster** | Cluster diagnostics | net_diag, sysinfo, osint |
| **Audio** | Audio analysis | audio, media_meta |
| **Recon** | OSINT and searching | osint, search, crate_scout |
| **Everything** | Everything at once | all installed plugins |

## JFP v5.4.0 Protocol

All plugins communicate via **JFP v5.4.0** standard over stdio:

```
[JFP_HEADER]
VERSION: v5.4.0
STATUS: OK          ← response (plugin → host)
INTENT: COMMAND     ← request (host → plugin)
[END_HEADER]
[MESSAGE_BODY]
command content
[END_MESSAGE]
```

## New plugin (forge.sh)

Create your own plugin in one command:

```bash
./forge.sh <name> [crate1 crate2 ...]

# Example:
./forge.sh recon reqwest scraper
```

This copies the template, adds dependencies, registers it in `workspace.toml`, and builds the plugin.

## Architecture

```
                    ┌─────────────┐
                    │ presets.toml │  ← domain presets
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

## License

MIT — see `LICENSE` file.

---

*Built for you — customize to your needs, develop, share!*
