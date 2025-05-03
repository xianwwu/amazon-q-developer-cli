use std::process::{
    Command,
    ExitCode,
};
use std::str::FromStr;

use anstream::{
    eprintln,
    println,
};
use clap::{
    Subcommand,
    ValueEnum,
};
use eyre::Result;

use crate::fig_util::manifest::FileType;

#[derive(Debug, ValueEnum, Clone, PartialEq, Eq)]
pub enum Build {
    Production,
    #[value(alias = "staging")]
    Beta,
    #[value(hide = true, alias = "dev")]
    Develop,
}

impl std::fmt::Display for Build {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Build::Production => f.write_str("production"),
            Build::Beta => f.write_str("beta"),
            Build::Develop => f.write_str("develop"),
        }
    }
}

#[derive(Debug, ValueEnum, Clone, PartialEq, Eq)]
pub enum App {
    Dashboard,
    Autocomplete,
}

impl std::fmt::Display for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            App::Dashboard => f.write_str("dashboard"),
            App::Autocomplete => f.write_str("autocomplete"),
        }
    }
}

#[derive(Debug, ValueEnum, Clone, PartialEq, Eq)]
pub enum AutocompleteWindowDebug {
    On,
    Off,
}

#[derive(Debug, ValueEnum, Clone, PartialEq, Eq)]
pub enum AccessibilityAction {
    Refresh,
    Reset,
    Prompt,
    Open,
    Status,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum TISAction {
    Enable,
    Disable,
    Select,
    Deselect,
}

#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum InputMethodDebugAction {
    Install {
        bundle_path: Option<PathBuf>,
    },
    Uninstall {
        bundle_path: Option<PathBuf>,
    },
    List,
    Status {
        bundle_path: Option<PathBuf>,
    },
    Source {
        bundle_identifier: String,
        #[arg(value_enum)]
        action: TISAction,
    },
}

#[derive(Debug, PartialEq, Subcommand)]
pub enum DebugSubcommand {
    /// Debug application codesigning
    #[cfg(target_os = "macos")]
    VerifyCodesign,
    /// Queries remote repository for updates given the specified metadata
    QueryIndex {
        #[arg(short, long)]
        channel: String,
        #[arg(short, long)]
        target_triple: String,
        #[arg(short = 'V', long)]
        variant: String,
        #[arg(short = 'e', long)]
        version: String,
        #[arg(short = 'r', long)]
        enable_rollout: bool,
        #[arg(short, long)]
        override_threshold: Option<u8>,
        #[arg(short, long)]
        file_type: String,
    },
    /// Displays remote index
    GetIndex {
        channel: String,
        #[arg(short, long, default_value = "false")]
        /// Display using debug formatting
        debug: bool,
    },
    RefreshAuthToken,
}

impl DebugSubcommand {
    pub async fn execute(&self) -> Result<ExitCode> {
        match self {
            #[cfg(target_os = "macos")]
            DebugSubcommand::VerifyCodesign => {
                Command::new("codesign")
                    .arg("-vvvv")
                    .arg(crate::fig_util::app_bundle_path())
                    .spawn()?
                    .wait()?;
            },
            DebugSubcommand::QueryIndex {
                channel,
                target_triple,
                variant,
                version: current_version,
                enable_rollout,
                override_threshold,
                file_type,
            } => {
                use crate::fig_util::manifest::{
                    Channel,
                    TargetTriple,
                    Variant,
                };

                let result = crate::fig_install::index::pull(&Channel::from_str(channel)?)
                    .await?
                    .find_next_version(
                        &TargetTriple::from_str(target_triple)?,
                        &Variant::from_str(variant)?,
                        Some(&FileType::from_str(file_type)?),
                        current_version,
                        !enable_rollout,
                        *override_threshold,
                    );

                println!("{result:#?}");
            },
            DebugSubcommand::GetIndex { channel, debug } => {
                use crate::fig_util::manifest::Channel;
                let index = crate::fig_install::index::pull(&Channel::from_str(channel)?).await?;
                if *debug {
                    println!("{index:#?}");
                } else {
                    let json = serde_json::to_string_pretty(&index)?;
                    println!("{json}");
                }
            },
            DebugSubcommand::RefreshAuthToken => match crate::fig_auth::refresh_token().await? {
                Some(_) => eprintln!("Refreshed token"),
                None => {
                    eprintln!("No token to refresh");
                    return Ok(ExitCode::FAILURE);
                },
            },
        }
        Ok(ExitCode::SUCCESS)
    }
}
