use std::path::PathBuf;

/// Abstraction used to instantiate the MCP servers.
/// This is consumed by the Multiplexer
pub struct Tool<'a> {
    name: &'a str,
    command: PathBuf,
    args: Vec<&'a str>,
}
