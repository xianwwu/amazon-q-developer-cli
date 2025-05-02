use cfg_if::cfg_if;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Failed to open URL")]
    Failed,
}

/// Returns bool indicating whether the URL was opened successfully
pub fn open_url(url: impl AsRef<str>) -> Result<(), Error> {
    cfg_if! {
        if #[cfg(target_os = "macos")] {
            open_macos(url)
        } else {
            match open_command(url).output() {
                Ok(output) => {
                    tracing::trace!(?output, "open_url output");
                    if output.status.success() {
                        Ok(())
                    } else {
                        Err(Error::Failed)
                    }
                },
                Err(err) => Err(err.into()),
            }
        }
    }
}

/// Returns bool indicating whether the URL was opened successfully
pub async fn open_url_async(url: impl AsRef<str>) -> Result<(), Error> {
    cfg_if! {
        if #[cfg(target_os = "macos")] {
            open_macos(url)
        } else {
            match tokio::process::Command::from(open_command(url)).output().await {
                Ok(output) => {
                    tracing::trace!(?output, "open_url_async output");
                    if output.status.success() {
                        Ok(())
                    } else {
                        Err(Error::Failed)
                    }
                },
                Err(err) => Err(err.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[test]
    fn test_open_url() {
        open_url("https://fig.io").unwrap();
    }
}
