use std::str::FromStr;
use std::time::SystemTimeError;

use thiserror::Error;
use tracing::error;

use crate::fig_util::manifest::{
    Channel,
    manifest,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error(transparent)]
    Util(#[from] crate::fig_util::Error),
    #[error(transparent)]
    Settings(#[from] crate::fig_settings::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Semver(#[from] semver::Error),
    #[error(transparent)]
    SystemTime(#[from] SystemTimeError),
    #[error(transparent)]
    Strum(#[from] strum::ParseError),
    #[error("failed to update: `{0}`")]
    UpdateFailed(String),
    #[cfg(target_os = "macos")]
    #[error("failed to update due to auth error: `{0}`")]
    SecurityFramework(#[from] security_framework::base::Error),
    #[error("your system is not supported on this channel")]
    SystemNotOnChannel,
    #[error("Update in progress")]
    UpdateInProgress,
    #[error("could not convert path to cstring")]
    Nul(#[from] std::ffi::NulError),
    #[error("failed to get system id")]
    SystemIdNotFound,
    #[error("unable to find the bundled metadata")]
    BundleMetadataNotFound,
}

use std::path::PathBuf;

use crate::fig_util::{
    CLI_BINARY_NAME,
    OLD_CLI_BINARY_NAMES,
    OLD_PTY_BINARY_NAMES,
    PTY_BINARY_NAME,
    directories,
};

pub async fn uninstall() -> Result<(), Error> {
    let remove_binary = |path: PathBuf| async move {
        match tokio::fs::remove_file(&path).await {
            Ok(_) => tracing::info!("Removed binary: {path:?}"),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {},
            Err(err) => tracing::warn!(%err, "Failed to remove binary: {path:?}"),
        }
    };

    // let folders = [directories::home_local_bin()?, Path::new("/usr/local/bin").into()];
    let folders = [directories::home_local_bin()?];

    let mut all_binary_names = vec![CLI_BINARY_NAME, PTY_BINARY_NAME];
    all_binary_names.extend(OLD_CLI_BINARY_NAMES);
    all_binary_names.extend(OLD_PTY_BINARY_NAMES);

    let mut pty_names = vec![PTY_BINARY_NAME];
    pty_names.extend(OLD_PTY_BINARY_NAMES);

    for folder in folders {
        for binary_name in &all_binary_names {
            let binary_path = folder.join(binary_name);
            remove_binary(binary_path).await;
        }
    }

    Ok(())
}

fn update() -> Result<(), Error> {
    // let status = self_update::backends::s3::Update::configure()
    //     .bucket_name("self_update_releases")
    //     .asset_prefix("something/self_update")
    //     .region("eu-west-2")
    //     .bin_name("self_update_example")
    //     .show_download_progress(true)
    //     .current_version(cargo_crate_version!())
    //     .build()?
    //     .update()?;
    // println!("S3 Update status: `{}`!", status.version());
    todo!();
}

impl From<crate::fig_util::directories::DirectoryError> for Error {
    fn from(err: crate::fig_util::directories::DirectoryError) -> Self {
        crate::fig_util::Error::Directory(err).into()
    }
}

// The current selected channel
pub fn get_channel() -> Result<Channel, Error> {
    Ok(match crate::fig_settings::state::get_string("updates.channel")? {
        Some(channel) => Channel::from_str(&channel)?,
        None => {
            let manifest_channel = manifest().default_channel;
            if crate::fig_settings::settings::get_bool_or("app.beta", false) {
                manifest_channel.max(Channel::Beta)
            } else {
                manifest_channel
            }
        },
    })
}
