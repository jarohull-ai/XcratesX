# Contributing

Start with an issue describing the concrete use case or bug, then open a focused pull
request with a reproducible example and validation results. Small contributions,
documentation corrections, and tests are welcome.

Use stable Rust on Linux. Run the four checks in README before submitting. Add regression
tests for behavior changes; tests should work offline and must not depend on external APIs.
Keep Cargo.lock committed. Format plugin_template separately when changing it.

Plugins must reserve stdout for JFP, log diagnostics to stderr, and remain responsive to
TICK during slow operations. Validate user input, set HTTP timeouts and response size
limits, and report failure as ERROR rather than successful output containing an error.
Never assume a separate process is a security boundary.

Document commands, dependencies, external requests, and limits. Keep example commands
consistent with tested behavior. Include new plugins in Cargo.toml and workspace.toml,
and add them to relevant presets. Do not commit tokens, private paths, or generated binaries.

The initial supported platform is Linux. Other operating systems require process lifecycle
and terminal integration work before being advertised as supported.
