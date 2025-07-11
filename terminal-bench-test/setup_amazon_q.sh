#!/bin/bash
set -e

echo "Installing Amazon Q for command line (Linux)..."
export AMAZON_Q_SIGV4=1
 
# Install the Q branch code into machine + dependencies
cd /home/runner/work/amazon-q-developer-cli/amazon-q-developer-cli/
npm run setup
