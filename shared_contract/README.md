# jfp-contract

Rust library implementing **JFP v5.4.0 (JARO Flash Protocol)** — a text-frame IPC contract for plugin-based TUI workbenches.

## What it does

- `JfpMessage` — parse, construct, and serialize JFP frames (header + body)
- `JfpReader` — streaming `BufRead` parser with configurable frame-size limit (default 1 MiB)
- Constants for protocol markers and version

## Frame format

```
[JFP_HEADER]
VERSION: v5.4.0
STATUS: OK
INTENT: COMMAND
[END_HEADER]
[MESSAGE_BODY]
payload text
[END_MESSAGE]
```

## Usage

The reader requires exactly one supported VERSION header and rejects oversized
frames (including oversized single lines). It recovers at the next frame header.
STATUS is used for replies; INTENT for requests. Flush stdout after each frame.
Diagnostics belong on stderr. Marker strings are reserved and must not appear in
payloads: this version does not implement escaping or arbitrary binary payloads.
Leading/trailing newlines in the body are trimmed by the parser.

```rust
use shared_contract::{JfpMessage, JfpReader};

// Build a reply
let msg = JfpMessage::reply("OK", "Hello from plugin");
print!("{}", msg.to_jfp_string());

// Read frames from stdin
let reader = std::io::stdin().lock();
let mut jfp = JfpReader::new(reader);
while let Some(frame) = jfp.next_frame() {
    println!("got: {:?}", frame);
}
```

## License

MIT
