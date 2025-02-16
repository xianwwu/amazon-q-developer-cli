use async_trait::async_trait;
use bstr::ByteSlice;
use fig_os_shim::Context;
use serde::Deserialize;
use std::{collections::HashMap, process::Stdio, sync::Arc};

use super::{Error, InvokeOutput, OutputKind, Tool};

const ALLOWED_OPS: [&str; 6] = ["get", "describe", "list", "ls", "search", "batch_get"];

#[derive(Debug, thiserror::Error)]
enum AwsToolError {
    ForbiddenOperation(String),
}

impl std::fmt::Display for AwsToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AwsToolError::ForbiddenOperation(op) => Ok(writeln!(f, "Forbidden operation encountered: {}", op)?),
        }
    }
}

// TODO: we should perhaps composite this struct with an interface that we can use to mock the
// actual cli with. That will allow us to more thoroughly test it.
#[derive(Debug)]
pub struct AwsTool {
    #[allow(dead_code)]
    ctx: Arc<Context>,
    pub args: AwsToolArgs,
}

impl std::fmt::Display for AwsTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Aws tool")?;
        self.args.fmt(f)?;

        Ok(())
    }
}

impl AwsTool {
    pub fn from_value(ctx: Arc<Context>, args: serde_json::Value) -> Result<Self, Error> {
        Ok(Self {
            ctx,
            args: serde_json::from_value(args)?,
        })
    }

    fn validate_operation(&self) -> Result<(), AwsToolError> {
        let operation_name = &self.args.operation_name;
        for op in ALLOWED_OPS {
            if self.args.operation_name.starts_with(op) {
                return Ok(());
            }
        }
        Err(AwsToolError::ForbiddenOperation(operation_name.clone()))
    }
}

#[async_trait]
impl Tool for AwsTool {
    async fn invoke(&self) -> Result<InvokeOutput, Error> {
        let AwsToolArgs {
            service_name,
            operation_name,
            parameters,
            region,
            profile_name,
            label: _,
        } = &self.args;
        self.validate_operation().map_err(|err| {
            Error::ToolInvocation(format!("Unable to spawn command '{} : {:?}'", self.args, err).into())
        })?;

        let mut command = tokio::process::Command::new("aws");
        command
            .envs(std::env::vars())
            .arg("--region")
            .arg(region)
            .arg("--profile")
            .arg(profile_name)
            .arg(service_name)
            .arg(operation_name);
        for (param_name, val) in parameters {
            if param_name.starts_with("--") {
                command.arg(param_name).arg(val);
            } else {
                command.arg(format!("--{}", param_name)).arg(val);
            }
        }
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                Error::ToolInvocation(format!("Unable to spawn command '{} : {:?}'", self.args, err).into())
            })?
            .wait_with_output()
            .await
            .map_err(|err| {
                Error::ToolInvocation(format!("Unable to spawn command '{} : {:?}'", self.args, err).into())
            })?;
        let status = output.status.code();
        let stdout = output.stdout.to_str_lossy();
        let stderr = output.stderr.to_str_lossy();

        Ok(InvokeOutput {
            output: OutputKind::Json(serde_json::json!({
                "exit_status": status,
                "stdout": stdout,
                "stderr": stderr
            })),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct AwsToolArgs {
    pub service_name: String,
    pub operation_name: String,
    pub parameters: HashMap<String, String>,
    pub region: String,
    pub profile_name: String,
    pub label: String,
}

impl std::fmt::Display for AwsToolArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Service name: {}", self.service_name)?;
        writeln!(f, "Operation name: {}", self.operation_name)?;
        writeln!(f, "Parameters: ")?;
        for (name, value) in &self.parameters {
            writeln!(f, "{}: {}", name, value)?;
        }
        writeln!(f, "Region: {}", self.region)?;
        writeln!(f, "Profile name: {}", self.profile_name)?;
        writeln!(f, "Label: {}", self.label)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_aws_read_only() {
        let ctx = Context::new_fake();

        let v = serde_json::json!({
            "service_name": "s3",
            "operation_name": "put-object",
            // technically this wouldn't be a valid request with an empty parameter set but it's
            // okay for this test
            "parameters": {},
            "region": "us-west-2",
            "profile_name": "default",
            "label": ""
        });
        let out = AwsTool::from_value(Arc::clone(&ctx), v).unwrap().invoke().await;
        assert!(out.is_err());
    }

    #[tokio::test]
    async fn test_aws_output() {
        let ctx = Context::new_fake();

        let v = serde_json::json!({
            "service_name": "s3",
            "operation_name": "ls",
            "parameters": {},
            "region": "us-west-2",
            "profile_name": "default",
            "label": ""
        });
        let out = AwsTool::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();

        if let OutputKind::Json(json) = out.output {
            // depending on where the test is ran we might get different outcome here but it does
            // not mean the tool is not working
            let exit_status = json.get("exit_status").unwrap();
            if exit_status == 0 {
                assert_eq!(json.get("stderr").unwrap(), "");
            } else {
                assert_ne!(json.get("stderr").unwrap(), "");
            }
        } else {
            panic!("Expected JSON output");
        }
    }

    #[tokio::test]
    async fn test_aws_command_with_params() {
        let ctx = Context::new_fake();

        let v = serde_json::json!({
            "service_name": "dynamodb",
            "operation_name": "get-item",
            "parameters": {
                "--table-name": "AGI_MEMORY",
                "--key": r#"{"memory_id": {"S": "49d649c7-b772-4578-968c-c20240844a4a"}}"#
            },
            "region": "us-west-2",
            "profile_name": "default",
            "label": ""
        });
        let out = AwsTool::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();

        if let OutputKind::Json(json) = out.output {
            // depending on where the test is ran we might get different outcome here but it does
            // not mean the tool is not working
            let exit_status = json.get("exit_status").unwrap();
            if exit_status == 0 {
                assert_eq!(json.get("stderr").unwrap(), "");
            } else {
                assert_ne!(json.get("stderr").unwrap(), "");
            }
        } else {
            panic!("Expected JSON output");
        }
    }
}
