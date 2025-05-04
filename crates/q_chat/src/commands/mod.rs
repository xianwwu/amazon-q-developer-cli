pub mod clear;
pub mod compact;
pub mod context;
pub mod context_adapter;
pub mod editor;
pub mod handler;
pub mod help;
pub mod issue;
pub mod profile;
pub mod quit;
pub mod test_utils;
pub mod tools;
pub mod usage;

pub use clear::ClearCommand;
pub use compact::CompactCommand;
pub use context::ContextCommand;
pub use context_adapter::CommandContextAdapter;
pub use editor::EditorCommand;
// Keep CommandHandler as crate-only visibility
pub(crate) use handler::CommandHandler;
pub use help::HelpCommand;
pub use issue::IssueCommand;
pub use profile::ProfileCommand;
pub use quit::QuitCommand;
pub use tools::ToolsCommand;
pub use usage::UsageCommand;
