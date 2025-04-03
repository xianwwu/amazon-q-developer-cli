# Customizing the Docker Setup for Amazon Q CLI

This guide provides detailed information for developers who want to customize or modify the Docker setup for Amazon Q CLI.

## Dockerfile Details

The default Dockerfile creates an Ubuntu-based container with:

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

## Architecture Support

The Dockerfile supports both x86_64 (amd64) and ARM64 architectures:

```bash
# For Intel/AMD processors
docker build --build-arg ARCH=amd64 -t amazon-q-dev .

# For ARM processors (M1/M2/M3 Macs, Graviton, etc.)
docker build --build-arg ARCH=arm64 -t amazon-q-dev .
```

## Data Storage Architecture

Amazon Q CLI stores its data in different locations:

| Purpose | Container Path | Storage Method |
|---------|---------------|----------------|
| **Main Data Directory** | `/home/dev/.local/share/amazon-q` | Docker volume: `amazon-q-data` |
| **Cache Directory** | `/home/dev/.cache/amazon-q` | Docker volume: `amazon-q-cache` |
| **AWS Profile Data** | `/home/dev/.aws/amazonq` | Host mount from `~/.aws/amazonq` |
| **Current Directory** | `/home/dev/src` | Host mount from current directory |

## Customizing the Docker Run Command

The default Docker run command used by the `q-dev` alias is:

```bash
docker run -it --rm \
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
  amazon-q-dev
```

You can customize this by:

1. **Adding resource limits**:
   ```bash
   docker run -it --rm \
     --cpus=2 \
     --memory=4g \
     ... # other options
   ```

2. **Adding additional volumes**:
   ```bash
   docker run -it --rm \
     -v "/path/to/host/dir:/path/in/container" \
     ... # other options
   ```

3. **Adding environment variables**:
   ```bash
   docker run -it --rm \
     -e ADDITIONAL_VAR=value \
     ... # other options
   ```

## Customizing the Setup Script

The `setup-q-dev-alias.sh` script can be modified to:

1. **Change the Docker image name**:
   ```bash
   # Find this line
   ALIAS_DEFINITION="alias q-dev='docker run -it --rm \
     ... # options
     amazon-q-dev'"
   
   # Change amazon-q-dev to your preferred image name
   ```

2. **Add additional volume mounts**:
   ```bash
   # Find the alias definition and add your volume mounts
   ALIAS_DEFINITION="alias q-dev='docker run -it --rm \
     ... # existing options
     -v \"/path/on/host:/path/in/container\" \
     amazon-q-dev'"
   ```

## Advanced Troubleshooting

### Database Issues

If you encounter database errors:

1. **Inspect the volume contents**:
   ```bash
   docker run --rm -it -v amazon-q-data:/data ubuntu ls -la /data
   ```

2. **Check database permissions**:
   ```bash
   q-dev --entrypoint bash
   ls -la ~/.local/share/amazon-q/
   ```

3. **Manually initialize the database**:
   ```bash
   q-dev --entrypoint bash
   sqlite3 ~/.local/share/amazon-q/data.sqlite3 "CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY);"
   ```

### Container Networking

By default, the container uses the default bridge network. To use a custom network:

```bash
# Create a custom network
docker network create q-network

# Run with custom network
docker run -it --rm --network q-network ... amazon-q-dev
```

## Creating a Custom Image

To create a custom image based on the default one:

```dockerfile
FROM amazon-q-dev

# Add your customizations
RUN apt-get update && apt-get install -y your-package
USER dev
RUN pip install your-python-package

# Override entrypoint if needed
ENTRYPOINT ["your-custom-entrypoint"]
CMD ["your-default-command"]
```

Build with:
```bash
docker build -t custom-q-dev -f CustomDockerfile .
```
