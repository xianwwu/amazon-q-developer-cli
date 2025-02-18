use std::collections::HashMap;
use std::fmt::Display;
use std::io::Stdout;
use std::process::Stdio;

use async_trait::async_trait;
use bstr::ByteSlice;
use fig_os_shim::Context;
use serde::Deserialize;

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
#[derive(Debug, Deserialize)]
pub struct UseAws {
    pub service_name: String,
    pub operation_name: String,
    pub parameters: HashMap<String, String>,
    pub region: String,
    pub profile_name: Option<String>,
    pub label: Option<String>,
}

impl UseAws {
    fn validate_operation(&self) -> Result<(), AwsToolError> {
        let operation_name = &self.operation_name;
        for op in ALLOWED_OPS {
            if self.operation_name.starts_with(op) {
                return Ok(());
            }
        }
        Err(AwsToolError::ForbiddenOperation(operation_name.clone()))
    }
}

#[async_trait]
impl Tool for UseAws {
    fn display_name(&self) -> String {
        "Use AWS".to_owned()
    }

    async fn invoke(&self, _: &Context, updates: Stdout) -> Result<InvokeOutput, Error> {
        self.validate_operation()
            .map_err(|err| Error::ToolInvocation(format!("Unable to spawn command '{} : {:?}'", self, err).into()))?;

        let mut command = tokio::process::Command::new("aws");
        let profile_name = if let Some(ref profile_name) = self.profile_name {
            profile_name
        } else {
            "default"
        };
        command
            .envs(std::env::vars())
            .arg("--region")
            .arg(&self.region)
            .arg("--profile")
            .arg(profile_name)
            .arg(&self.service_name)
            .arg(&self.operation_name);
        for (param_name, val) in &self.parameters {
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
            .map_err(|err| Error::ToolInvocation(format!("Unable to spawn command '{} : {:?}'", self, err).into()))?
            .wait_with_output()
            .await
            .map_err(|err| Error::ToolInvocation(format!("Unable to spawn command '{} : {:?}'", self, err).into()))?;
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

    async fn show_readable_intention(&self) -> Result<(), Error> {
        crossterm::queue!(
            std::io::stdout(),
            crossterm::style::Print("Running aws cli command:\n"),
            crossterm::style::Print(format!("Service name: {}\n", self.service_name)),
            crossterm::style::Print(format!("Operation name: {}\n", self.operation_name)),
            crossterm::style::Print("Parameters: \n".to_string()),
        )?;
        for (name, value) in &self.parameters {
            crossterm::queue!(
                std::io::stdout(),
                crossterm::style::Print(format!("{}: {}\n", name, value))
            )?;
        }

        if let Some(ref profile_name) = self.profile_name {
            crossterm::queue!(
                std::io::stdout(),
                crossterm::style::Print(format!("Profile name: {}\n", profile_name))
            )?;
        } else {
            crossterm::queue!(
                std::io::stdout(),
                crossterm::style::Print("Profile name: default\n".to_string())
            )?;
        }

        crossterm::queue!(
            std::io::stdout(),
            crossterm::style::Print(format!("Region: {}\n", self.region))
        )?;

        if let Some(ref label) = self.label {
            crossterm::queue!(
                std::io::stdout(),
                crossterm::style::Print(format!("Label: {}\n", label))
            )?;
        }

        Ok(())
    }

    async fn validate(&mut self, _ctx: &Context) -> Result<(), Error> {
        self.validate_operation()
            .map_err(|err| Error::ToolInvocation(format!("Unable to spawn command '{} : {:?}'", self, err).into()))?;

        Ok(())
    }
}

impl Display for UseAws {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crossterm::queue!(
            std::io::stdout(),
            crossterm::style::Print("Running aws cli command:\n"),
            crossterm::style::Print(format!(
                "{} with operation {}\n",
                self.service_name, self.operation_name
            ))
        )
        .map_err(|_| std::fmt::Error)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::stdout;

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

        assert!(
            serde_json::from_value::<UseAws>(v)
                .unwrap()
                .invoke(&ctx, stdout())
                .await
                .is_err()
        );
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
        let out = serde_json::from_value::<UseAws>(v)
            .unwrap()
            .invoke(&ctx, stdout())
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
        let out = serde_json::from_value::<UseAws>(v)
            .unwrap()
            .invoke(&ctx, stdout())
            .await
            .unwrap();

        if let OutputKind::Json(json) = out.output {
            // depending on where the test is ran we might get different outcome here but it does
            // not mean the tool is not working
            let exit_status = json.get("exit_status").unwrap();
            if exit_status == 0 {
                assert_eq!(json.get("stderr").unwrap(), "");
                println!("query result: {}", json.get("stdout").unwrap());
            } else {
                assert_ne!(json.get("stderr").unwrap(), "");
            }
        } else {
            panic!("Expected JSON output");
        }
    }
}
