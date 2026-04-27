#!/bin/bash

set -e

cd "$(dirname "$0")"

echo "Starting Insighta CLI installation..."

if ! command -v cargo &> /dev/null; then
    echo "Rust not found. Installing Rust toolchain..."
    
    if command -v curl &> /dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        
        if [ -f "$HOME/.cargo/env" ]; then
            source "$HOME/.cargo/env"
        fi

        echo "Rust installed successfully."
    else
        echo "Error: 'curl' is required to install Rust. Please install curl and try again."
        exit 1
    fi
else
    echo "Rust is already installed."
fi

if command -v insighta &> /dev/null; then
    echo "'insighta' is already installed at $(command -v insighta)"
    read -p "Do you want to reinstall/update to the latest version? (y/N) " confirm < /dev/tty
    if [[ $confirm != [yY] && $confirm != [yY][eE][sS] ]]; then
        echo "Skipping installation. You're all set!"
        exit 0
    fi
fi

echo "Building and installing insighta CLI..."

# Using --force to ensure it updates if already installed
cargo install --path . --force

if command -v insighta &> /dev/null; then
    echo "Success! 'insighta' is now available in your PATH."
else
    [ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
    
    if command -v insighta &> /dev/null; then
        echo "Success! 'insighta' is now available in your PATH."
    else
        echo "Installation finished, but 'insighta' is not yet in your PATH."
        echo "Please run: source \$HOME/.cargo/env"
        echo "Or restart your terminal."
    fi
fi

echo "Done! Try running: insighta --help"
