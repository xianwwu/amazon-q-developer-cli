# Docker Setup for Amazon Q CLI Chat

This guide explains how to set up a Docker container for Amazon Q CLI Chat development, with proper data persistence using Docker volumes.

## Dockerfile

Create a `Dockerfile` in your project directory:

```dockerfile
# Use Ubuntu with the appropriate architecture
ARG ARCH=amd64
FROM --platform=linux/${ARCH} ubuntu:latest

# Set environment variables (timezone will be set at runtime)
ENV DEBIAN_FRONTEND=noninteractive TERM="xterm-256color"

# Install common dependencies and mise
RUN apt-get update && apt-get install -y curl wget git jq vim nano unzip zip ssh ca-certificates \
    gnupg lsb-release software-properties-common build-essential pkg-config tzdata sqlite3 && \
    rm -rf /var/lib/apt/lists/*

# Install AWS CLI and Amazon Q CLI with architecture detection
ARG TARGETARCH
RUN ARCH_AWS=$([ "$TARGETARCH" = "amd64" ] && echo "x86_64" || echo "aarch64") && \
    curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-$ARCH_AWS.zip" -o awscliv2.zip && \
    curl --proto '=https' --tlsv1.2 -sSf "https://desktop-release.codewhisperer.us-east-1.amazonaws.com/latest/q-$ARCH_AWS-linux.zip" -o q.zip && \
    unzip awscliv2.zip && ./aws/install && rm -rf awscliv2.zip ./aws && \
    unzip q.zip -d /tmp && chmod +x /tmp/q/install.sh && Q_INSTALL_GLOBAL=true /tmp/q/install.sh && \
    rm -rf q.zip /tmp/q

# Create non-root user and set up directories
RUN useradd -ms /bin/bash dev && \
    mkdir -p /home/dev/src /home/dev/.aws/amazonq/profiles /home/dev/.ssh && \
    mkdir -p /home/dev/.local/share/amazon-q /home/dev/.cache/amazon-q && \
    chown -R dev:dev /home/dev

# Switch to non-root user
USER dev
WORKDIR /home/dev/src

RUN echo 'export PS1="\[\033[01;32m\]q-dev\[\033[00m\]:\[\033[01;34m\]\w\[\033[00m\]\$ "' >> /home/dev/.bashrc

# Initialize the SQLite database to ensure proper permissions
RUN mkdir -p /home/dev/.local/share/amazon-q && \
    touch /home/dev/.local/share/amazon-q/data.sqlite3 && \
    chmod 644 /home/dev/.local/share/amazon-q/data.sqlite3

# Install mise for managing multiple language runtimes
RUN curl https://mise.run | sh && \
    echo 'eval "$(~/.local/bin/mise activate --shims bash)"' >> ~/.bashrc && \
    eval "$(~/.local/bin/mise activate --shims bash)" && \
    ~/.local/bin/mise use -g python@latest uv@latest node@lts java@latest go@latest

# Default command starts Amazon Q chat
ENTRYPOINT [ "q" ]
CMD ["chat"]
```

## Quick Setup

The easiest way to set up is to use our setup script:

```bash
# Download the setup script
curl -O https://raw.githubusercontent.com/aws/amazon-q-developer-cli/main/docs/docker/setup-q-dev-alias.sh

# Make it executable
chmod +x setup-q-dev-alias.sh

# Source it to add the alias to your current session
source ./setup-q-dev-alias.sh

# Or force rebuild the Docker image
source ./setup-q-dev-alias.sh --rebuild
```

## Manual Setup

### Building the Docker Image

Build the image with:

```bash
# Detect architecture automatically
ARCH=$(uname -m | sed 's/x86_64/amd64/' | sed 's/arm64\|aarch64/arm64/')
docker build --build-arg ARCH=$ARCH -t amazon-q-dev .
```

Or specify the architecture explicitly:

```bash
# For Intel/AMD processors
docker build --build-arg ARCH=amd64 -t amazon-q-dev .

# For ARM processors (M1/M2/M3 Macs, Graviton, etc.)
docker build --build-arg ARCH=arm64 -t amazon-q-dev .
```

### Creating Docker Volumes

Create persistent volumes for Amazon Q data:

```bash
docker volume create amazon-q-data
docker volume create amazon-q-cache
```

### Setting Up the Alias

Create a shell alias for easy access:

```bash
alias q-dev='docker run -it --rm \
  -v "$(pwd):/home/dev/src" \
  -v "${HOME}/.aws/credentials:/home/dev/.aws/credentials:ro" \
  -v "${HOME}/.aws/config:/home/dev/.aws/config:ro" \
  -v "${HOME}/.aws/amazonq:/home/dev/.aws/amazonq:rw" \
  -v amazon-q-data:/home/dev/.local/share/amazon-q \
  -v amazon-q-cache:/home/dev/.cache/amazon-q \
  -v "${HOME}/.gitconfig:/home/dev/.gitconfig:ro" \
  -v "${HOME}/.ssh:/home/dev/.ssh:ro" \
  -e AWS_PROFILE \
  -e AWS_REGION \
  -e TZ \
  amazon-q-dev'
```

## Understanding the Directory Mapping

Amazon Q CLI stores its data in different locations:

| Purpose | Container Path | Storage Method |
|---------|---------------|----------------|
| **Main Data Directory** | `/home/dev/.local/share/amazon-q` | Docker volume: `amazon-q-data` |
| **Cache Directory** | `/home/dev/.cache/amazon-q` | Docker volume: `amazon-q-cache` |
| **AWS Profile Data** | `/home/dev/.aws/amazonq` | Host mount from `~/.aws/amazonq` |
| **Current Directory** | `/home/dev/src` | Host mount from current directory |

Using Docker volumes for the data and cache directories ensures:
1. Persistence across container restarts
2. No issues with paths containing spaces
3. Better performance
4. Proper isolation

## Usage

After setting up the alias:

1. Navigate to your project directory
2. Run the container:
   ```bash
   q-dev
   ```
3. Amazon Q chat will start automatically
4. Use context commands to add project files:
   ```
   /context add README.md
   ```

## Resource Allocation

For better performance, you can allocate more resources by adding these parameters to your Docker run command:

```bash
--cpus=2 \
--memory=4g \
```

## Troubleshooting

If you encounter database errors like:

```
Failed to open database: timed out waiting for connection: unable to open database file: /home/dev/.local/share/amazon-q/data.sqlite3
```

Try these solutions:

1. **Reset the Docker volumes**:
   ```bash
   docker volume rm amazon-q-data amazon-q-cache
   docker volume create amazon-q-data
   docker volume create amazon-q-cache
   ```

2. **Check permissions inside the container**:
   ```bash
   q-dev --entrypoint bash
   ls -la ~/.local/share/amazon-q/
   ```

3. **Initialize the database manually**:
   ```bash
   q-dev --entrypoint bash
   touch ~/.local/share/amazon-q/data.sqlite3
   chmod 644 ~/.local/share/amazon-q/data.sqlite3
   ```

4. **Verify SQLite is installed**:
   ```bash
   q-dev --entrypoint bash
   sqlite3 --version
   ```

## Running with Bash Instead of Q Chat

To start a bash shell instead of Q chat:

```bash
q-dev --entrypoint bash
```
