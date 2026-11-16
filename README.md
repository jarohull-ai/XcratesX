# XcratesX

A Linux terminal workbench for Rust crate experiments and independent command-line plugins.
Status: experimental. MIT licensed.

XcratesX runs one selected plugin at a time. Presets filter the available plugins;
switching plugins stops the previous process and resets its state. Plugins exchange
JFP messages over standard input/output. Separate processes contain ordinary crashes,
but are **not a security sandbox**.

## Quick start

Install stable Rust with Cargo, a C/C++ linker, Git, and standard Linux build tools.
Some dependencies you experiment with may require additional system libraries.

```sh
git clone https://github.com/jarohull-ai/XcratesX.git
cd XcratesX
cargo build --workspace --release --locked
./target/release/host_engine
```

Choose a preset using arrows and Enter. Use the slot keys shown in the footer to
select a plugin: 1–9, s (web search), c (intentional crash test).
Tab enters command mode; Enter submits; Esc leaves command mode.
Outside command mode: p opens presets, r reloads the full slot configuration,
h toggles the last 32 distinct responses, q exits. Arrows/PageUp/PageDown scroll output.
Reload currently restores all slots, even after selecting a narrower preset.

## Real crate experiments

In Meccano, choose assembler (4), press Tab, and enter:

```text
test:serde:1, serde_json:1
```

This creates an independent temporary Cargo project and builds both dependencies
together. Requirements follow Cargo semver syntax: `1` permits compatible updates;
`=1.0.100` pins an exact version. Bare crate names use `*`.
Up to 32 unique crates are accepted. Commas or newlines separate entries; requirements
containing commas are not supported. The result includes the resolved Cargo.lock.
Successful compilation does not prove API compatibility or application correctness.

Choose analyzer (6):

```text
analyze serde:1, tokio:1
analyze path /absolute/path/to/project/Cargo.toml
```

The analyzer reads Cargo's actual resolved graph, listing versioned packages and
their dependency edges. Local project analysis uses `--locked`: create/update that
project's lockfile yourself first. Metadata covers the Cargo resolution graph, not
a source-level call graph or a target-specific execution graph.

Cargo jobs run in the background, with one job per plugin. Output is limited to
8 MiB per stream and Cargo is stopped after 180 seconds. Switching plugins stops
the plugin process group, including Cargo descendants. Temporary experiment projects
are normally removed after completion; forced termination can leave directories
named `xcratesx-*` in the OS temporary directory.
For cached dependencies only, launch with `XCRATESX_OFFLINE=1`.

Building crates executes their build scripts and procedural macros with your user
permissions. Use trusted crates, or run the whole workbench in a disposable container
without secrets. No Git dependencies, custom features, or source-code generation
are currently supported by the assembler.

## Plugins

| Slot | Plugin | Purpose |
| --- | --- | --- |
| 1 | crate_scout | crates.io search and package details |
| 2 | sysinfo | system telemetry |
| 3 | net_diag | network diagnostics |
| 4 | crate_assembler | compile dependency combinations |
| 5 | crate_kit | static, curated name/description catalog |
| 6 | crate_analyzer | real Cargo dependency graph |
| 7 | audio | WAV measurements |
| 8 | media_meta | audio metadata |
| 9 | osint | IP/domain lookup through ip-api.com |
| s | search | fetch and inspect HTML |
| c | crucible | intentional failure for host testing |

Network plugins contact external services; availability, quotas and API policies
are outside this project's control. OSINT sends the requested target to ip-api.com.
The catalog is a starting point, not a guarantee of crate maintenance or quality.
Some existing plugin messages and UI labels remain Polish.

## Configuration and diagnostics

`workspace.toml` registers slots; `presets.toml` groups them. Slot keys must be unique,
single ASCII letters/digits; q, p, r and h are reserved. Binaries must be safe filenames
and reside beside host_engine.

The default configuration is found relative to the Cargo build directory, then the
working directory. Set `XCRATESX_CONFIG_DIR` to an absolute directory for a custom installation.
Set `XCRATESX_LOG_DIR` to choose the plugin stderr directory. Otherwise logs live in
`$XDG_STATE_HOME/xcratesx`, falling back to `~/.local/state/xcratesx`.
Each plugin's stderr log is replaced when it starts. Use h to inspect response history.

## Contributing plugins

```sh
./forge.sh my_tool
# Optional dependencies:
./forge.sh my_http_tool reqwest
cargo build --workspace
```

The generator creates a package from plugin_template, registers a free keyboard
shortcut, and builds it in debug mode. Use the debug host while developing, or rebuild
the release workspace before running the release host. Failed builds leave the scaffold
available for inspection.

See [CONTRIBUTING.md](CONTRIBUTING.md), [protocol documentation](shared_contract/README.md),
and [security notes](SECURITY.md).

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

CI runs these checks on Ubuntu. Tests include protocol recovery, configuration consistency,
real child-process messaging/shutdown, and an offline Cargo fixture. Live API behavior
is not covered by those tests.

## License

[MIT](LICENSE).
