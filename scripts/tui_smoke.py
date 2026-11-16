#!/usr/bin/env python3
"""Exercise global experiment commands through the real Linux terminal UI."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time

ROOT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="xcratesx-tui-") as directory:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    env = dict(os.environ, TERM="xterm", XCRATESX_DATA_DIR=directory,
               XCRATESX_LOG_DIR=directory, XCRATESX_OFFLINE="1")
    process = subprocess.Popen([str(ROOT / "target/debug/host_engine")],
                               stdin=slave, stdout=slave, stderr=slave, env=env, cwd=ROOT)
    os.close(slave)
    def expect(text, timeout=60):
        output = b""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if select.select([master], [], [], .1)[0]:
                try:
                    output += os.read(master, 65536)
                except OSError:
                    break
                if text.encode() in output:
                    return
            if process.poll() is not None:
                break
        raise AssertionError(f"Missing {text!r}: {output[-3000:]!r}")
    def command(text):
        os.write(master, ("\t" + text + "\r").encode())
    def saved_file(name, contains=None, timeout=60):
        path = Path(directory) / "terminal-demo" / name
        deadline = time.monotonic() + timeout
        output = b""
        while time.monotonic() < deadline:
            if select.select([master], [], [], .1)[0]:
                output += os.read(master, 65536)
            if path.exists() and (contains is None or contains in path.read_text()):
                return
            if process.poll() is not None:
                break
        raise AssertionError(f"Expected saved {name}: {output[-3000:]!r}")
    try:
        expect("Choose your workspace")
        os.write(master, b"2")
        expect("EXPERIMENTS")
        command(":new terminal-demo")
        saved_file("dependencies.toml")
        command(":add serde:1, serde_json:1")
        saved_file("dependencies.toml", "serde_json")
        command(":build")
        saved_file("result.txt", "BUILD OK")
        command(":graph")
        saved_file("graph.txt", "Resolved packages:")
        command(":show")
        os.write(master, b"q")
        assert process.wait(timeout=5) == 0
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)
print("Terminal experiment workflow: PASS")
