#!/usr/bin/env bash
set -e

echo "Building and installing collab..."

# Default: build without monitor (works on Rust 1.85+, no textual-rs needed)
# Add --features monitor if you want the live TUI monitor (requires Rust 1.88+)
cargo install --path collab-cli --force
cargo install --path collab-server --force

echo ""
echo "Done. 'collab' and 'collab-server' are now on your PATH."
echo ""
echo "Configure: create ~/.collab.toml"
echo "  host = \"http://your-server:8000\""
echo "  instance = \"your-worker-name\""
echo "  recipients = [\"other-worker\"]"
echo ""
echo "Run 'collab config-path' to confirm the config file location."
