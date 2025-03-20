use std::collections::HashMap;
use std::sync::Arc;

use convert_case::Casing;
use fig_api_client::model::{
    ToolResult,
    ToolResultContentBlock,
    ToolResultStatus,
};
use futures::{
    StreamExt,
    stream,
};
use serde::{
    Deserialize,
    Serialize,
};

use super::parser::ToolUse;
use super::tools::Tool;
use super::tools::custom_tool::{
    CustomToolClient,
    CustomToolConfig,
};
use super::tools::execute_bash::ExecuteBash;
use super::tools::fs_read::FsRead;
use super::tools::fs_write::FsWrite;
use super::tools::use_aws::UseAws;
use crate::cli::chat::tools::ToolSpec;
use crate::cli::chat::tools::custom_tool::CustomTool;

// This is to mirror claude's config set up
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    mcp_servers: HashMap<String, CustomToolConfig>,
}

pub struct ToolManager {
    clients: HashMap<String, Arc<CustomToolClient>>,
}

impl ToolManager {
    pub async fn from_configs(config: McpServerConfig) -> Self {
        let McpServerConfig { mcp_servers } = config;
        let pre_initialized = mcp_servers
            .into_iter()
            .map(|(server_name, server_config)| {
                let server_name = server_name.to_case(convert_case::Case::Snake);
                let custom_tool_client = CustomToolClient::from_config(server_name.clone(), server_config);
                (server_name, custom_tool_client)
            })
            .collect::<Vec<(String, _)>>();
        let init_results = stream::iter(pre_initialized)
            .map(|(name, uninit_client)| async move { (name, uninit_client.await) })
            .buffer_unordered(10)
            .collect::<Vec<(String, _)>>()
            .await;
        let mut clients = HashMap::<String, Arc<CustomToolClient>>::new();
        for (name, init_res) in init_results {
            match init_res {
                Ok(client) => {
                    tracing::info!("################ Initialized server with name {}", name);
                    clients.insert(name, Arc::new(client));
                },
                Err(e) => {
                    // TODO: log this
                    tracing::info!("################ Error initializing mcp server: {:?}", e.to_string());
                },
            }
        }
        Self { clients }
    }

    pub async fn load_tools(&self) -> eyre::Result<HashMap<String, ToolSpec>> {
        let mut tool_specs =
            serde_json::from_str::<HashMap<String, ToolSpec>>(include_str!("tools/tool_index.json"))?;
        for client in self.clients.values() {
            match client.get_tool_spec().await {
                Ok((name, specs)) => {
                    // Each mcp server might have multiple tools. 
                    // To avoid naming conflicts we are going to namespace it.
                    // This would also help us locate which mcp server to call the tool from.
                    for mut spec in specs {
                        spec.name = format!("{}__0__{}", name, spec.name);
                        tool_specs.insert(spec.name.clone(), spec);
                    }
                },
                Err(e) => {
                    // TODO: log this. Perhaps also delete it from the list of tools we have?
                    tracing::info!("################ Error loading tool: {:?}", e.to_string());
                }
            }
        }
        Ok(tool_specs)
    }

    pub fn get_tool_from_tool_use(&self, value: ToolUse) -> Result<Tool, ToolResult> {
        tracing::info!("############### tool use received: {:#?}", value);
        let map_err = |parse_error| ToolResult {
            tool_use_id: value.id.clone(),
            content: vec![ToolResultContentBlock::Text(format!(
                "Failed to validate tool parameters: {parse_error}. The model has either suggested tool parameters which are incompatible with the existing tools, or has suggested one or more tool that does not exist in the list of known tools."
            ))],
            status: ToolResultStatus::Error,
        };

        Ok(match value.name.as_str() {
            "fs_read" => Tool::FsRead(serde_json::from_value::<FsRead>(value.args).map_err(map_err)?),
            "fs_write" => Tool::FsWrite(serde_json::from_value::<FsWrite>(value.args).map_err(map_err)?),
            "execute_bash" => Tool::ExecuteBash(serde_json::from_value::<ExecuteBash>(value.args).map_err(map_err)?),
            "use_aws" => Tool::UseAws(serde_json::from_value::<UseAws>(value.args).map_err(map_err)?),
            // Note that this name is namespaced with server_name__0__tool_name
            name => {
                let (server_name, tool_name) = name.split_once("__0__").ok_or(ToolResult {
                        tool_use_id: value.id.clone(),
                        content: vec![ToolResultContentBlock::Text(format!(
                            "The tool, \"{name}\" is supplied with incorrect name"
                        ))],
                        status: ToolResultStatus::Error,
                })?;
                let Some(client) = self.clients.get(server_name) else {
                    return Err(ToolResult {
                        tool_use_id: value.id,
                        content: vec![ToolResultContentBlock::Text(format!(
                            "The tool, \"{server_name}\" is not supported by the client"
                        ))],
                        status: ToolResultStatus::Error,
                    });
                };
                let custom_tool = CustomTool {
                    name: tool_name.to_owned(),
                    client: client.clone(),
                    method: "tools/call".to_owned(),
                    params: Some(value.args),
                };
                Tool::Custom(custom_tool)
            },
        })
    }
}
