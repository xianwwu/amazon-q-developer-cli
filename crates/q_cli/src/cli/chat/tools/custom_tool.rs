use std::io::Write;
use std::sync::Arc;

use eyre::Result;
use fig_os_shim::Context;
use mcp_client::{
    Client as McpClient,
    StdioTransport,
};

use super::InvokeOutput;

#[derive(Clone, Debug)]
pub enum CustomToolClient {
    Stdio { client: Arc<McpClient<StdioTransport>> },
}

impl CustomToolClient {
    pub async fn request(&self) -> () {
        todo!()
    }

    pub async fn notify(&self) -> () {
        todo!()
    }
}

pub struct CustomTool {
    client: CustomToolClient,
}

impl CustomTool {
    pub async fn invoke(&self, ctx: &Context, updates: &mut impl Write) -> Result<InvokeOutput> {
        todo!()
    }

    pub fn queue_description(&self, updates: &mut impl Write) -> Result<()> {
        todo!()
    }

    pub async fn validate(&mut self, ctx: &Context) -> Result<()> {
        todo!()
    }
}
