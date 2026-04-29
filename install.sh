#!/bin/bash
set -e

echo ""
echo "👊 Installing Punch..."
echo ""

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Build and install CLI
echo "Building punch CLI..."
cd core
cargo install --path . --quiet
cd ..

echo ""
echo "✅ Punch installed!"
echo ""
echo "Try it:"
echo "  punch generate         → get a code"
echo "  punch connect <code>   → connect to a peer"
echo ""
echo "Note: Punch works best on WiFi."
echo "Mobile/corporate networks may fall back to relay."
echo ""
