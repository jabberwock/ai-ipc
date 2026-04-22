#!/usr/bin/env bash
set -e

echo "Building and installing collab..."

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
