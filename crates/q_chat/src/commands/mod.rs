mod clear;
// mod compact;
pub mod context;
pub mod handler;
mod help;
// pub mod profile;
mod quit;
pub mod registry;
// #[cfg(test)]
// pub mod test_utils;
// pub mod tools;

pub use clear::ClearCommand;
// pub use compact::CompactCommand;
pub use context::ContextCommand;
pub use handler::CommandHandler;
pub use help::HelpCommand;
// pub use profile::ProfileCommand;
pub use quit::QuitCommand;
pub use registry::CommandRegistry;
// pub use tools::ToolsCommand;

// We'll uncomment these as we implement each command
