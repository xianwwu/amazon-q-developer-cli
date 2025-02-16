use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use bstr::ByteSlice;
use eyre::Result;
use fig_os_shim::Context;
use serde::Deserialize;

use super::{Error, InvokeOutput, OutputKind, Tool};

#[derive(Debug)]
pub struct ExecuteBash {
    // todo - add process mocking to Context?
    #[allow(dead_code)]
    ctx: Arc<Context>,
    pub args: ExecuteBashArgs,
}

impl ExecuteBash {
    pub fn from_value(ctx: Arc<Context>, args: serde_json::Value) -> Result<Self, Error> {
        Ok(Self {
            ctx,
            args: serde_json::from_value(args)?,
        })
    }
}

impl std::fmt::Display for ExecuteBash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Execute Bash Command")?;
        writeln!(f, "- Command: `{}`", self.args.command)?;
        Ok(())
    }
}

#[async_trait]
impl Tool for ExecuteBash {
    async fn invoke(&self) -> Result<InvokeOutput, Error> {
        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&self.args.command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                Error::ToolInvocation(format!("Unable to spawn command '{}': {:?}", &self.args.command, err).into())
            })?
            .wait_with_output()
            .await
            .map_err(|err| {
                Error::ToolInvocation(
                    format!(
                        "Unable to wait on subprocess for command '{}': {:?}",
                        &self.args.command, err
                    )
                    .into(),
                )
            })?;
        let status = output.status.code();
        let stdout = output.stdout.to_str_lossy();
        let stderr = output.stderr.to_str_lossy();
        Ok(InvokeOutput {
            output: OutputKind::Json(serde_json::json!({
                "exit_status": status,
                "stdout": stdout,
                "stderr": stderr,
            })),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ExecuteBashArgs {
    pub command: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_bash_tool() {
        let ctx = Context::new_fake();

        // Verifying stdout
        let v = serde_json::json!({
            "command": "echo Hello, world!"
        });
        let out = ExecuteBash::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();

        if let OutputKind::Json(json) = out.output {
            assert_eq!(json.get("exit_status").unwrap(), 0);
            assert_eq!(json.get("stdout").unwrap(), "Hello, world!\n");
            assert_eq!(json.get("stderr").unwrap(), "");
        } else {
            panic!("Expected JSON output");
        }

        // Verifying stderr
        let v = serde_json::json!({
            "command": "echo Hello, world! 1>&2"
        });
        let out = ExecuteBash::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();

        if let OutputKind::Json(json) = out.output {
            assert_eq!(json.get("exit_status").unwrap(), 0);
            assert_eq!(json.get("stdout").unwrap(), "");
            assert_eq!(json.get("stderr").unwrap(), "Hello, world!\n");
        } else {
            panic!("Expected JSON output");
        }

        // Verifying exit code
        let v = serde_json::json!({
            "command": "exit 1"
        });
        let out = ExecuteBash::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();

        if let OutputKind::Json(json) = out.output {
            assert_eq!(json.get("exit_status").unwrap(), 1);
            assert_eq!(json.get("stdout").unwrap(), "");
            assert_eq!(json.get("stderr").unwrap(), "");
        } else {
            panic!("Expected JSON output");
        }
    }
}
