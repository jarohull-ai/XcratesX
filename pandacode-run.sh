#!/bin/sh
set -eu
cd "$(dirname "$0")"
cargo build
exec ./target/debug/host_engine
