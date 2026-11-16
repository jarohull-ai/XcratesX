# Security

XcratesX is experimental and has not undergone an independent security audit.
Plugins and Cargo experiments run with the current user's permissions. Process groups
help terminate jobs; they do not restrict filesystem or network access.

Only run trusted plugins and crates. Cargo builds may run build.rs scripts and procedural
macros. For unfamiliar code use a disposable VM/container without credentials or sensitive
mounts. Network plugins send user-supplied targets to the documented service or destination.

JFP input frames are bounded and require the supported version. Host input, plugin logging,
and third-party workloads are not a comprehensive resource sandbox.

For a sensitive vulnerability, use GitHub private vulnerability reporting if enabled on
the repository. Otherwise open a minimal issue requesting a private contact without
publishing exploit details or credentials. A response-time SLA is not currently offered.
