use async_trait::async_trait;
use tracing::warn;

use super::{
    InvokeOutput,
    Tool,
    Error,
};

#[derive(Debug)]
pub struct Custom {}

impl std::fmt::Display for Custom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Custom Tool")?;
        Ok(())
    }
}

#[async_trait]
impl Tool for Custom {
    async fn invoke(&self) -> Result<InvokeOutput, Error> {
        warn!("Not implemented");
        Ok(Default::default())
    }
}
