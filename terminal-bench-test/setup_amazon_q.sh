#!/bin/bash
set -e

echo "Installing Amazon Q for command line (Linux)..."

# Update package lists and install required tools
apt-get update
apt-get install -y curl unzip sqlite3 bc
export AMAZON_Q_SIGV4=1

# Check glibc version and architecture
GLIBC_VERSION=$(ldd --version | head -n1 | grep -o '[0-9]\+\.[0-9]\+' | head -n1)
ARCH=$(uname -m)

echo "Detected architecture: $ARCH"
echo "Detected glibc version: $GLIBC_VERSION"

# Determine download URL based on architecture and glibc version
if [ "$ARCH" = "x86_64" ]; then
    if [ "$(echo "$GLIBC_VERSION >= 2.34" | bc -l 2>/dev/null || echo 0)" = "1" ]; then
        Q_URL="https://desktop-release.q.us-east-1.amazonaws.com/latest/q-x86_64-linux.zip"
        echo "Using standard x86_64 version"
    else
        Q_URL="https://desktop-release.q.us-east-1.amazonaws.com/latest/q-x86_64-linux-musl.zip"
        echo "Using musl x86_64 version for older glibc"
    fi
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    if [ "$(echo "$GLIBC_VERSION >= 2.34" | bc -l 2>/dev/null || echo 0)" = "1" ]; then
        Q_URL="https://desktop-release.q.us-east-1.amazonaws.com/latest/q-aarch64-linux.zip"
        echo "Using standard aarch64 version"
    else
        Q_URL="https://desktop-release.q.us-east-1.amazonaws.com/latest/q-aarch64-linux-musl.zip"
        echo "Using musl aarch64 version for older glibc"
    fi
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

# Download Amazon Q
echo "Downloading from: $Q_URL"
curl --proto '=https' --tlsv1.2 -sSf "$Q_URL" -o "q.zip"

# Extract and install w/o  interactive installer
unzip q.zip
cp q/bin/q /usr/local/bin/q
cp q/bin/qchat /usr/local/bin/qchat
chmod +x /usr/local/bin/q
chmod +x /usr/local/bin/qchat

# Test qchat installation without hanging
echo "Testing qchat version..."
q --version
echo "Cleaning q zip"
rm -f q.zip
