use std::path::PathBuf;

use crate::fig_install::Error;
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
