use std::process::ExitCode;

use anstream::println;
use clap::Args;
use crossterm::style::Stylize;
use eyre::Result;

use crate::fig_install::index::UpdatePackage;
use crate::fig_install::{
    UpdateOptions,
    UpdateStatus,
};
use crate::fig_os_shim::{
    Context,
    Os,
};
use crate::fig_util::CLI_BINARY_NAME;
use crate::fig_util::manifest::{
    Variant,
    manifest,
};

#[derive(Debug, PartialEq, Args)]
pub struct UpdateArgs {
    /// Don't prompt for confirmation
    #[arg(long, short = 'y')]
    non_interactive: bool,
    /// Relaunch into dashboard after update (false will launch in background)
    #[arg(long, default_value = "true")]
    relaunch_dashboard: bool,
    /// Uses rollout
    #[arg(long)]
    rollout: bool,
}

impl UpdateArgs {
    pub async fn execute(&self) -> Result<ExitCode> {
        let ctx = Context::new();
        if ctx.platform().os() == Os::Linux && manifest().variant == Variant::Full {
            return try_linux_update().await;
        }

        let UpdateArgs {
            non_interactive,
            relaunch_dashboard,
            rollout,
        } = &self;

        let res = crate::fig_install::update(
            Context::new(),
            Some(Box::new(|mut recv| {
                tokio::runtime::Handle::current().spawn(async move {
                    let progress_bar = indicatif::ProgressBar::new(100);
                    loop {
                        match recv.recv().await {
                            Some(UpdateStatus::Percent(p)) => {
                                progress_bar.set_position(p as u64);
                            },
                            Some(UpdateStatus::Message(m)) => {
                                progress_bar.set_message(m);
                            },
                            Some(UpdateStatus::Error(e)) => {
                                progress_bar.abandon();
                                return Err(eyre::eyre!(e));
                            },
                            Some(UpdateStatus::Exit) | None => {
                                progress_bar.finish_with_message("Done!");
                                break;
                            },
                        }
                    }
                    Ok(())
                });
            })),
            UpdateOptions {
                ignore_rollout: !rollout,
                interactive: !non_interactive,
                relaunch_dashboard: *relaunch_dashboard,
            },
        )
        .await;

        match res {
            Ok(true) => Ok(ExitCode::SUCCESS),
            Ok(false) => {
                println!(
                    "No updates available, \n{} is the latest version.",
                    env!("CARGO_PKG_VERSION").bold()
                );
                Ok(ExitCode::SUCCESS)
            },
            Err(err) => eyre::bail!(
                "{err}\n\nIf this is unexpected, try running {} and then try again.\n",
                format!("{CLI_BINARY_NAME} doctor").bold()
            ),
        }
    }
}

async fn try_linux_update() -> Result<ExitCode> {
    display_update_check_result(&crate::fig_install::check_for_updates(true).await)
}

fn display_update_check_result(
    check_for_updates_result: &Result<Option<UpdatePackage>, crate::fig_install::Error>,
) -> Result<ExitCode> {
    match check_for_updates_result {
        Ok(Some(pkg)) => {
            println!("A new version of {} is available: {}", CLI_BINARY_NAME, pkg.version);
            Ok(ExitCode::SUCCESS)
        },
        Ok(None) => {
            println!(
                "No updates available, \n{} is the latest version.",
                env!("CARGO_PKG_VERSION").bold()
            );
            Ok(ExitCode::SUCCESS)
        },
        Err(err) => {
            eyre::bail!(
                "{err}\n\nFailed checking for updates. If this is unexpected, try running {} and then try again.\n",
                format!("{CLI_BINARY_NAME} doctor").bold()
            )
        },
    }
}
