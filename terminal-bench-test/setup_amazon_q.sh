#!/bin/bash
set -e
# if git hash empty then set to latest auto
git_hash=${GITHUB_SHA:+"$(git rev-parse --short "$GITHUB_SHA")"}
git_hash=${git_hash:-"latest"}
apt-get update
apt-get install -y curl wget unzip jq

echo "Installing AWS CLI..."
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip -q awscliv2.zip
./aws/install --bin-dir /usr/local/bin --install-dir /usr/local/aws-cli

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


# Save original credentials
ORIGINAL_AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}
ORIGINAL_AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}
ORIGINAL_AWS_SESSION_TOKEN=${AWS_SESSION_TOKEN}

# Assume role and capture temporary credentials --> needed for s3 bucket access for build
echo "Assuming role FigIoChat-S3Access-Role-Gamma..."
TEMP_CREDENTIALS=$(aws sts assume-role --role-arn arn:aws:iam::${FIGCHAT_GAMMA_ID}:role/FigIoChat-S3Access-Role-Gamma --role-session-name S3AccessSession)

# Extract and export temporary credentials -> jq is just used 
export AWS_ACCESS_KEY_ID=$(echo $TEMP_CREDENTIALS | jq -r '.Credentials.AccessKeyId')
export AWS_SECRET_ACCESS_KEY=$(echo $TEMP_CREDENTIALS | jq -r '.Credentials.SecretAccessKey')
export AWS_SESSION_TOKEN=$(echo $TEMP_CREDENTIALS | jq -r '.Credentials.SessionToken')

# Download specific build from S3 based on commit hash
echo "Downloading Amazon Q CLI build from S3..."
S3_BUCKET="fig-io-chat-build-output-${FIGCHAT_GAMMA_ID}-us-east-1"
S3_PREFIX="main/${git_hash}/x86_64-unknown-linux-musl"
echo "Downloading qchat.zip from s3://${S3_BUCKET}/${S3_PREFIX}/qchat.zip"
aws s3 cp s3://${S3_BUCKET}/${S3_PREFIX}/qchat.zip ./qchat.zip --region us-east-1


# Handle the zip file, copy the qchat executable to /usr/local/bin + symlink from old code
echo "Extracting qchat.zip..."
unzip -q qchat.zip
mkdir -p /usr/local/bin
cp -f ./qchat/qchat /usr/local/bin/qchat
chmod +x /usr/local/bin/qchat
ln -sf /usr/local/bin/qchat /usr/local/bin/q

# Restore credentials to run Q
export AWS_ACCESS_KEY_ID=${ORIGINAL_AWS_ACCESS_KEY_ID}
export AWS_SECRET_ACCESS_KEY=${ORIGINAL_AWS_SECRET_ACCESS_KEY}
export AWS_SESSION_TOKEN=${ORIGINAL_AWS_SESSION_TOKEN}

cat > ~/.aws/credentials << EOF
[default]
aws_access_key_id = ${ORIGINAL_AWS_ACCESS_KEY_ID}
aws_secret_access_key = ${ORIGINAL_AWS_SECRET_ACCESS_KEY}
aws_session_token = ${ORIGINAL_AWS_SESSION_TOKEN}
EOF

echo "Cleaning q zip"
rm -f qchat.zip
rm -rf qchat






