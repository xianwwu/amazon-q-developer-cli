#!/bin/bash
set -e
apt-get update
apt-get install -y curl wget gnupg2 software-properties-common git

# Install Node.js and npm
curl -fsSL https://deb.nodesource.com/setup_18.x | bash -
apt-get install -y nodejs
apt-get install -y build-essential gcc g++ make
apt-get install -y pkg-config libssl-dev

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
echo "Amazon Q CLI installation completed successfully"


# Create AWS credentials from environment variables
mkdir -p ~/.aws
cat > ~/.aws/credentials << EOF
[default]
aws_access_key_id = ${AWS_ACCESS_KEY_ID}
aws_secret_access_key = ${AWS_SECRET_ACCESS_KEY}
aws_session_token = ${AWS_SESSION_TOKEN}
EOF
chmod 600 ~/.aws/credentials

cat > ~/.aws/config << EOF
[default]
region = us-east-1
EOF
chmod 600 ~/.aws/config