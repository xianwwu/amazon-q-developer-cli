## Docker Setup for Amazon Q CLI Chat

This guide explains how to set up a Docker container for Amazon Q CLI Chat development.

## Dockerfile

Create a `Dockerfile` in your project directory:

```dockerfile
# Use latest Ubuntu
FROM ubuntu:latest

# Set environment variables (timezone will be set at runtime)
ENV DEBIAN_FRONTEND=noninteractive TERM="xterm-256color"

# Install common dependencies and mise
RUN apt-get update && apt-get install -y curl wget git jq vim nano unzip zip ssh ca-certificates \
    gnupg lsb-release software-properties-common build-essential pkg-config tzdata && \
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
    mkdir -p /home/dev/src /home/dev/.aws/amazonq /home/dev/.ssh && \
    chown -R dev:dev /home/dev

# Switch to non-root user
USER dev
WORKDIR /home/dev/src

# Install mise for managing multiple language runtimes
RUN curl https://mise.run | sh && \
    echo 'eval "$(~/.local/bin/mise activate bash)"' >> ~/.bashrc

# Set up mise for development languages
RUN ~/.local/bin/mise use -g python@latest node@lts java@latest go@latest && \
    echo 'export PS1="\[\033[01;32m\]q-dev\[\033[00m\]:\[\033[01;34m\]\w\[\033[00m\]\$ "' >> /home/dev/.bashrc

# Default command starts Amazon Q chat
ENTRYPOINT [ "q" ]
CMD ["chat"]
```

## Building the Docker Image

Build the image with:

```bash
docker build -t amazon-q-dev .
```

## Running the Container

Create a shell alias for easy access:

```bash
alias q-dev='docker run -it --rm \
  -v "$(pwd):/home/dev/src" \
  -v "${HOME}/.aws:/home/dev/.aws:rw" \
  -v "${HOME}/.gitconfig:/home/dev/.gitconfig:ro" \
  -v "${HOME}/.ssh:/home/dev/.ssh:ro" \
  -e AWS_PROFILE \
  -e AWS_REGION \
  -e TZ=$(date +%Z) \
  amazon-q-dev'
```

## Understanding the Setup

- **Development Environment**: The container includes multiple language runtimes (Python, Node.js, Java, Ruby, Go) managed by mise
- **AWS Integration**: AWS CLI and Amazon Q CLI are pre-installed
- **Configuration Sharing**: Your AWS credentials, Git config, and SSH keys are mounted from the host
- **Current Directory Access**: Your current working directory is mounted as the container's working directory
- **Host Timezone**: The container uses your host machine's timezone
- **Default Command**: The container automatically starts `q chat` when launched

## Usage

After setting up the alias and reloading your shell configuration:

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

For better performance, allocate more resources:

```bash
alias q-dev='docker run -it --rm \
  --cpus=2 \
  --memory=4g \
  -v "$(pwd):/home/dev/src" \
  -v "${HOME}/.aws/credentials:/home/dev/.aws/credentials:ro" \
  -v "${HOME}/.aws/config:/home/dev/.aws/config:ro" \
  -v "${HOME}/.gitconfig:/home/dev/.gitconfig:ro" \
  -v "${HOME}/.ssh:/home/dev/.ssh:ro" \
  -e AWS_PROFILE \
  -e AWS_REGION \
  -e TZ=$(date +%Z) \
  amazon-q-dev'
```
