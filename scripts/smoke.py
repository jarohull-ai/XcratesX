#!/usr/bin/env python3
"""Offline end-to-end test of saved experiments through the public CLI."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("XCRATESX_TEST_BINARY", ROOT / "target/debug/host_engine"))

with tempfile.TemporaryDirectory(prefix="xcratesx-e2e-") as directory:
    env = dict(os.environ, XCRATESX_DATA_DIR=directory, XCRATESX_OFFLINE="1")
    def command(text, success=True):
        result = subprocess.run([str(BINARY), "--experiment", text], env=env,
                                text=True, capture_output=True, timeout=200)
        assert (result.returncode == 0) == success, result.stdout + result.stderr
        return result.stdout + result.stderr

    assert "Created demo" in command(":new demo")
    command(":new demo", success=False)
    command(":new ../escape", success=False)
    command(":add serde:1, serde_json:1")
    assert "BUILD OK" in command(":build")
    saved = Path(directory) / "demo"
    lock = (saved / "Cargo.lock").read_bytes()
    assert "serde_json@" in command(":graph")
    assert lock == (saved / "Cargo.lock").read_bytes(), "Analysis changed locked versions"
    command(":new another")
    command(":use demo")
    assert "BUILD OK" in command(":show")
    assert "BUILD OK" in command(":build")
    assert lock == (saved / "Cargo.lock").read_bytes(), "Rebuild changed locked versions"
    # Saved directories can be shared and compiled with standard Cargo.
    subprocess.run(["cargo", "check", "--offline", "--locked",
                    "--manifest-path", str(saved / "Cargo.toml")],
                   check=True, capture_output=True, timeout=200)
    command(":remove serde_json")
    assert not (saved / "Cargo.lock").exists()
    assert "Not built since last edit" in command(":show")
    command(":use nonexistent", success=False)
    assert "demo" in command(":list")
print("Saved-experiment workflow: PASS")
