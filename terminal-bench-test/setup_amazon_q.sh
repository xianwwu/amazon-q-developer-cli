#!/bin/bash
set -e
apt-get update
apt-get install -y curl wget gnupg2 software-properties-common git

# Install Node.js and npm
curl -fsSL https://deb.nodesource.com/setup_18.x | bash -
apt-get install -y nodejs

# Install Rust and Cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# get the github code
echo "Installing Amazon Q for command line (Linux)..."
mkdir -p /app/amazon-q-developer-cli
cd /app
export AMAZON_Q_SIGV4=1
git clone https://github.com/aws/amazon-q-developer-cli.git
cd amazon-q-developer-cli
npm run setup

echo "Amazon Q CLI installation completed successfully"
