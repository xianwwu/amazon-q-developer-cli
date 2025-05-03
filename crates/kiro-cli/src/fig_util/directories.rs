use std::path::PathBuf;

use thiserror::Error;

use crate::fig_os_shim::{
    EnvProvider,
    FsProvider,
    Os,
    Shim,
};
use crate::fig_util::env_var::Q_PARENT;

#[derive(Debug, Error)]
pub enum DirectoryError {
    #[error("home directory not found")]
    NoHomeDirectory,
    #[error("runtime directory not found: neither XDG_RUNTIME_DIR nor TMPDIR were found")]
    NoRuntimeDirectory,
    #[error("non absolute path: {0:?}")]
    NonAbsolutePath(PathBuf),
    #[error("unsupported platform: {0:?}")]
    UnsupportedOs(Os),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    TimeFormat(#[from] time::error::Format),
    #[error(transparent)]
    Utf8FromPath(#[from] camino::FromPathError),
    #[error(transparent)]
    Utf8FromPathBuf(#[from] camino::FromPathBufError),
    #[error(transparent)]
    FromVecWithNul(#[from] std::ffi::FromVecWithNulError),
    #[error(transparent)]
    IntoString(#[from] std::ffi::IntoStringError),
    #[error("{Q_PARENT} env variable not set")]
    QParentNotSet,
    #[error("must be ran from an appimage executable")]
    NotAppImage,
}

type Result<T, E = DirectoryError> = std::result::Result<T, E>;

/// The directory of the users home
///
/// - Linux: /home/Alice
/// - MacOS: /Users/Alice
/// - Windows: C:\Users\Alice
pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or(DirectoryError::NoHomeDirectory)
}

pub fn home_dir_ctx<Ctx: FsProvider + EnvProvider>(ctx: &Ctx) -> Result<PathBuf> {
    if ctx.env().is_real() {
        home_dir()
    } else {
        ctx.env()
            .get("HOME")
            .map_err(|_err| DirectoryError::NoHomeDirectory)
            .and_then(|h| {
                if h.is_empty() {
                    Err(DirectoryError::NoHomeDirectory)
                } else {
                    Ok(h)
                }
            })
            .map(PathBuf::from)
            .map(|p| ctx.fs().chroot_path(p))
    }
}

/// The directory of the users `$HOME/.local/bin` directory
///
/// MacOS and Linux path: `$HOME/.local/bin``
#[cfg(unix)]
pub fn home_local_bin() -> Result<PathBuf> {
    let mut path = home_dir()?;
    path.push(".local/bin");
    Ok(path)
}

#[cfg(target_os = "linux")]
pub fn home_local_bin_ctx(ctx: &Context) -> Result<PathBuf> {
    let mut path = home_dir_ctx(ctx)?;
    path.push(".local/bin");
    Ok(path)
}

/// The q data directory
///
/// - Linux: `$XDG_DATA_HOME/amazon-q` or `$HOME/.local/share/amazon-q`
/// - MacOS: `$HOME/Library/Application Support/amazon-q`
pub fn fig_data_dir() -> Result<PathBuf> {
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            Ok(dirs::data_local_dir()
                .ok_or(DirectoryError::NoHomeDirectory)?
                .join("amazon-q"))
        } else if #[cfg(windows)] {
            Ok(fig_dir()?.join("userdata"))
        }
    }
}

pub fn fig_data_dir_ctx(fs: &impl FsProvider) -> Result<PathBuf> {
    Ok(fs.fs().chroot_path(fig_data_dir()?))
}

/// Get the macos tempdir from the `confstr` function
///
/// See: <https://man7.org/linux/man-pages/man3/confstr.3.html>
#[cfg(target_os = "macos")]
fn macos_tempdir() -> Result<PathBuf> {
    let len = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, std::ptr::null::<i8>().cast_mut(), 0) };
    let mut buf: Vec<u8> = vec![0; len];
    unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, buf.as_mut_ptr().cast(), buf.len()) };
    let c_string = std::ffi::CString::from_vec_with_nul(buf)?;
    let str = c_string.into_string()?;
    Ok(PathBuf::from(str))
}

/// Runtime dir is used for runtime data that should not be persisted for a long time, e.g. socket
/// files and logs
///
/// The XDG_RUNTIME_DIR is set by systemd <https://www.freedesktop.org/software/systemd/man/latest/file-hierarchy.html#/run/user/>,
/// if this is not set such as on macOS it will fallback to TMPDIR which is secure on macOS
#[cfg(unix)]
pub fn runtime_dir() -> Result<PathBuf> {
    let mut dir = dirs::runtime_dir();
    dir = dir.or_else(|| std::env::var_os("TMPDIR").map(PathBuf::from));

    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            let macos_tempdir = macos_tempdir()?;
            dir = dir.or(Some(macos_tempdir));
        } else {
            dir = dir.or_else(|| Some(std::env::temp_dir()));
        }
    }

    dir.ok_or(DirectoryError::NoRuntimeDirectory)
}

/// The directory to all the fig logs
/// - Linux: `/tmp/fig/$USER/logs`
/// - MacOS: `$TMPDIR/logs`
/// - Windows: `%TEMP%\fig\logs`
pub fn logs_dir() -> Result<PathBuf> {
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            use crate::CLI_BINARY_NAME;
            Ok(runtime_dir()?.join(format!("{CLI_BINARY_NAME}log")))
        } else if #[cfg(windows)] {
            Ok(std::env::temp_dir().join("amazon-q").join("logs"))
        }
    }
}

/// The directory to the directory containing config for the `/context` feature in `q chat`.
pub fn chat_global_context_path<Ctx: FsProvider + EnvProvider>(ctx: &Ctx) -> Result<PathBuf> {
    Ok(home_dir_ctx(ctx)?
        .join(".aws")
        .join("amazonq")
        .join("global_context.json"))
}

/// The directory to the directory containing config for the `/context` feature in `q chat`.
pub fn chat_profiles_dir<Ctx: FsProvider + EnvProvider>(ctx: &Ctx) -> Result<PathBuf> {
    Ok(home_dir_ctx(ctx)?.join(".aws").join("amazonq").join("profiles"))
}

/// The path to the fig settings file
pub fn settings_path() -> Result<PathBuf> {
    Ok(fig_data_dir()?.join("settings.json"))
}

/// The path to the lock file used to indicate that the app is updating
pub fn update_lock_path(ctx: &impl FsProvider) -> Result<PathBuf> {
    Ok(fig_data_dir_ctx(ctx)?.join("update.lock"))
}

#[cfg(test)]
mod linux_tests {
    use super::*;

    #[test]
    fn all_paths() {
        let ctx = crate::fig_os_shim::Context::new();
        assert!(logs_dir().is_ok());
        assert!(settings_path().is_ok());
        assert!(update_lock_path(&ctx).is_ok());
    }
}

// TODO(grant): Add back path tests on linux
#[cfg(all(test, not(target_os = "linux")))]
mod tests {
    use insta;

    use super::*;

    macro_rules! assert_directory {
        ($value:expr, @$snapshot:literal) => {
            insta::assert_snapshot!(
                sanitized_directory_path($value),
                @$snapshot,
            )
        };
    }

    macro_rules! macos {
        ($value:expr, @$snapshot:literal) => {
            #[cfg(target_os = "macos")]
            assert_directory!($value, @$snapshot)
        };
    }

    macro_rules! linux {
        ($value:expr, @$snapshot:literal) => {
            #[cfg(target_os = "linux")]
            assert_directory!($value, @$snapshot)
        };
    }

    macro_rules! windows {
        ($value:expr, @$snapshot:literal) => {
            #[cfg(target_os = "windows")]
            assert_directory!($value, @$snapshot)
        };
    }

    fn sanitized_directory_path(path: Result<PathBuf>) -> String {
        let mut path = path.unwrap().into_os_string().into_string().unwrap();

        if let Ok(home) = std::env::var("HOME") {
            let home = home.strip_suffix('/').unwrap_or(&home);
            path = path.replace(home, "$HOME");
        }

        let user = whoami::username();
        path = path.replace(&user, "$USER");

        if let Ok(tmpdir) = std::env::var("TMPDIR") {
            let tmpdir = tmpdir.strip_suffix('/').unwrap_or(&tmpdir);
            path = path.replace(tmpdir, "$TMPDIR");
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(tmpdir) = macos_tempdir() {
                let tmpdir = tmpdir.to_str().unwrap();
                let tmpdir = tmpdir.strip_suffix('/').unwrap_or(tmpdir);
                path = path.replace(tmpdir, "$TMPDIR");
            };
        }

        if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            let xdg_runtime_dir = xdg_runtime_dir.strip_suffix('/').unwrap_or(&xdg_runtime_dir);
            path = path.replace(xdg_runtime_dir, "$XDG_RUNTIME_DIR");
        }

        #[cfg(target_os = "linux")]
        {
            path = path.replace("/tmp", "$TMPDIR");
        }

        path
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_home_local_bin() {
        linux!(home_local_bin(), @"$HOME/.local/bin");
        macos!(home_local_bin(), @"$HOME/.local/bin");
    }

    #[test]
    fn snapshot_fig_data_dir() {
        linux!(fig_data_dir(), @"$HOME/.local/share/amazon-q");
        macos!(fig_data_dir(), @"$HOME/Library/Application Support/amazon-q");
        windows!(fig_data_dir(), @r"C:\Users\$USER\AppData\Local\Fig\userdata");
    }

    #[test]
    fn snapshot_settings_path() {
        linux!(settings_path(), @"$HOME/.local/share/amazon-q/settings.json");
        macos!(settings_path(), @"$HOME/Library/Application Support/amazon-q/settings.json");
        windows!(settings_path(), @r"C:\Users\$USER\AppData\Lcoal\Fig\settings.json");
    }

    #[test]
    fn snapshot_update_lock_path() {
        let ctx = crate::fig_os_shim::Context::new();
        linux!(update_lock_path(&ctx), @"$HOME/.local/share/amazon-q/update.lock");
        macos!(update_lock_path(&ctx), @"$HOME/Library/Application Support/amazon-q/update.lock");
        windows!(update_lock_path(&ctx), @r"C:\Users\$USER\AppData\Local\Fig\userdata\update.lock");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_tempdir_test() {
        let tmpdir = macos_tempdir().unwrap();
        println!("{:?}", tmpdir);
    }
}
