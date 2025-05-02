mod cli_context;
pub mod desktop;
pub mod directories;
pub mod manifest;
mod open;
pub mod pid_file;
pub mod process_info;
mod region_check;
mod shell;
pub mod spinner;
pub mod system_info;
pub mod terminal;

pub mod consts;
#[cfg(target_os = "macos")]
pub mod launchd_plist;

use std::cmp::Ordering;
use std::env;
use std::ffi::OsStr;
use std::fmt::Display;
use std::io::{
    ErrorKind,
    stdout,
};
use std::path::{
    Path,
    PathBuf,
};
use std::process::Command;

use anstream::stream::IsTerminal;
use cfg_if::cfg_if;
pub use cli_context::CliContext;
pub use consts::*;
use crossterm::style::Stylize;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use eyre::{
    Context,
    ContextCompat,
    Result,
    bail,
};
use globset::{
    Glob,
    GlobSet,
    GlobSetBuilder,
};
pub use open::{
    open_url,
    open_url_async,
};
pub use process_info::get_parent_process_exe;
use rand::Rng;
use regex::Regex;
pub use region_check::region_check;
pub use shell::Shell;
pub use terminal::Terminal;
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io operation error")]
    IoError(#[from] std::io::Error),
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("unsupported architecture")]
    UnsupportedArch,
    #[error(transparent)]
    Directory(#[from] directories::DirectoryError),
    #[error("process has no parent")]
    NoParentProcess,
    #[error("could not find the os hwid")]
    HwidNotFound,
    #[error("the shell, `{0}`, isn't supported yet")]
    UnknownShell(String),
    #[error("missing environment variable `{0}`")]
    MissingEnv(&'static str),
    #[error("unknown display server `{0}`")]
    UnknownDisplayServer(String),
    #[error("unknown desktop, checked environment variables: {0}")]
    UnknownDesktop(UnknownDesktopErrContext),
    #[error(transparent)]
    StrUtf8Error(#[from] std::str::Utf8Error),
    #[error("Failed to parse shell {0} version")]
    ShellVersion(Shell),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct UnknownDesktopErrContext {
    xdg_current_desktop: String,
    xdg_session_desktop: String,
    gdm_session: String,
}

impl std::fmt::Display for UnknownDesktopErrContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XDG_CURRENT_DESKTOP: `{}`, ", self.xdg_current_desktop)?;
        write!(f, "XDG_SESSION_DESKTOP: `{}`, ", self.xdg_session_desktop)?;
        write!(f, "GDMSESSION: `{}`", self.gdm_session)
    }
}

/// Returns a random 64 character hex string
///
/// # Example
///
/// ```
/// use fig_util::gen_hex_string;
///
/// let hex = gen_hex_string();
/// assert_eq!(hex.len(), 64);
/// ```
pub fn gen_hex_string() -> String {
    let mut buf = [0u8; 32];
    rand::rng().fill(&mut buf);
    hex::encode(buf)
}

pub fn search_xdg_data_dirs(ext: impl AsRef<std::path::Path>) -> Option<PathBuf> {
    let ext = ext.as_ref();
    if let Ok(xdg_data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for base in xdg_data_dirs.split(':') {
            let check = Path::new(base).join(ext);
            if check.exists() {
                return Some(check);
            }
        }
    }
    None
}

/// Returns the path to the original executable, not the symlink
pub fn current_exe_origin() -> Result<PathBuf, Error> {
    Ok(std::env::current_exe()?.canonicalize()?)
}

#[must_use]
fn app_bundle_path_opt() -> Option<PathBuf> {
    use consts::macos::BUNDLE_CONTENTS_MACOS_PATH;

    let current_exe = current_exe_origin().ok()?;

    // Verify we have .../Bundle.app/Contents/MacOS/binary-name
    let mut parts: PathBuf = current_exe.components().rev().skip(1).take(3).collect();
    parts = parts.iter().rev().collect();

    if parts != Path::new(APP_BUNDLE_NAME).join(BUNDLE_CONTENTS_MACOS_PATH) {
        return None;
    }

    // .../Bundle.app/Contents/MacOS/binary-name -> .../Bundle.app
    current_exe.ancestors().nth(3).map(|s| s.into())
}

#[must_use]
pub fn app_bundle_path() -> PathBuf {
    app_bundle_path_opt().unwrap_or_else(|| Path::new("/Applications").join(APP_BUNDLE_NAME))
}

pub fn partitioned_compare(lhs: &str, rhs: &str, by: char) -> Ordering {
    let sides = lhs
        .split(by)
        .filter(|x| !x.is_empty())
        .zip(rhs.split(by).filter(|x| !x.is_empty()));

    for (lhs, rhs) in sides {
        match if lhs.chars().all(|x| x.is_numeric()) && rhs.chars().all(|x| x.is_numeric()) {
            // perform a numerical comparison
            let lhs: u64 = lhs.parse().unwrap();
            let rhs: u64 = rhs.parse().unwrap();
            lhs.cmp(&rhs)
        } else {
            // perform a lexical comparison
            lhs.cmp(rhs)
        } {
            Ordering::Equal => continue,
            s => return s,
        }
    }

    lhs.len().cmp(&rhs.len())
}

/// Glob patterns against full paths
pub fn glob_dir(glob: &GlobSet, directory: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    // List files in the directory
    let dir = std::fs::read_dir(directory)?;

    for entry in dir {
        let path = entry?.path();

        // Check if the file matches the glob pattern
        if glob.is_match(&path) {
            files.push(path);
        }
    }

    Ok(files)
}

/// Glob patterns against the file name
pub fn glob_files(glob: &GlobSet, directory: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    // List files in the directory
    let dir = std::fs::read_dir(directory)?;

    for entry in dir {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name();

        // Check if the file matches the glob pattern
        if let Some(file_name) = file_name {
            if glob.is_match(file_name) {
                files.push(path);
            }
        }
    }

    Ok(files)
}

pub fn glob<I, S>(patterns: I) -> Result<GlobSet>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern.as_ref())?);
    }
    Ok(builder.build()?)
}

pub fn app_path_from_bundle_id(bundle_id: impl AsRef<OsStr>) -> Option<String> {
    cfg_if! {
        if #[cfg(target_os = "macos")] {
            let installed_apps = std::process::Command::new("mdfind")
                .arg("kMDItemCFBundleIdentifier")
                .arg("=")
                .arg(bundle_id)
                .output()
                .ok()?;

            let path = String::from_utf8_lossy(&installed_apps.stdout);
            Some(path.trim().split('\n').next()?.into())
        } else {
            let _bundle_id = bundle_id;
            None
        }
    }
}

pub fn is_executable_in_path(program: impl AsRef<Path>) -> bool {
    match env::var_os("PATH") {
        Some(path) => env::split_paths(&path).any(|p| p.join(&program).is_file()),
        _ => false,
    }
}

pub fn app_not_running_message() -> String {
    format!(
        "\n{}\n{PRODUCT_NAME} app might not be running, to launch {PRODUCT_NAME} run: {}\n",
        format!("Unable to connect to {PRODUCT_NAME} app").bold(),
        format!("{CLI_BINARY_NAME} launch").magenta()
    )
}

pub fn login_message() -> String {
    format!(
        "{}\nLooks like you aren't logged in to {PRODUCT_NAME}, to login run: {}",
        "Not logged in".bold(),
        format!("{CLI_BINARY_NAME} login").magenta()
    )
}

pub fn match_regex(regex: impl AsRef<str>, input: impl AsRef<str>) -> Option<String> {
    Some(
        Regex::new(regex.as_ref())
            .unwrap()
            .captures(input.as_ref())?
            .get(1)?
            .as_str()
            .into(),
    )
}

pub fn choose(prompt: impl Display, options: &[impl ToString]) -> Result<Option<usize>> {
    if options.is_empty() {
        bail!("no options passed to choose")
    }

    if !stdout().is_terminal() {
        warn!("called choose while stdout is not a terminal");
        return Ok(Some(0));
    }

    match Select::with_theme(&dialoguer_theme())
        .items(options)
        .default(0)
        .with_prompt(prompt.to_string())
        .interact_opt()
    {
        Ok(ok) => Ok(ok),
        Err(dialoguer::Error::IO(io)) if io.kind() == ErrorKind::Interrupted => Ok(None),
        Err(e) => Err(e).wrap_err("Failed to choose"),
    }
}

pub fn input(prompt: &str, initial_text: Option<&str>) -> Result<String> {
    if !stdout().is_terminal() {
        warn!("called input while stdout is not a terminal");
        return Ok(String::new());
    }

    let theme = dialoguer_theme();
    let mut input = dialoguer::Input::with_theme(&theme).with_prompt(prompt);

    if let Some(initial_text) = initial_text {
        input = input.with_initial_text(initial_text);
    }

    Ok(input.interact_text()?)
}

pub fn get_running_app_info(bundle_id: impl AsRef<str>, field: impl AsRef<str>) -> Result<String> {
    let info = Command::new("lsappinfo")
        .args(["info", "-only", field.as_ref(), "-app", bundle_id.as_ref()])
        .output()?;
    let info = String::from_utf8(info.stdout)?;
    let value = info
        .split('=')
        .nth(1)
        .context(eyre::eyre!("Could not get field value for {}", field.as_ref()))?
        .replace('"', "");
    Ok(value.trim().into())
}

pub fn get_app_info() -> Result<String> {
    let output = Command::new("lsappinfo")
        .args(["info", "-app", APP_BUNDLE_ID])
        .output()?;
    let result = String::from_utf8(output.stdout)?;
    Ok(result.trim().into())
}

pub fn dialoguer_theme() -> ColorfulTheme {
    ColorfulTheme {
        prompt_prefix: dialoguer::console::style("?".into()).for_stderr().magenta(),
        ..ColorfulTheme::default()
    }
}

#[cfg(target_os = "macos")]
pub async fn is_brew_reinstall() -> bool {
    let regex = regex::bytes::Regex::new(r"brew(\.\w+)?\s+(upgrade|reinstall|install)").unwrap();

    tokio::process::Command::new("ps")
        .args(["aux", "-o", "args"])
        .output()
        .await
        .is_ok_and(|output| regex.is_match(&output.stdout))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn regex() {
        let regex_test = |regex: &str, input: &str, expected: Option<&str>| {
            assert_eq!(match_regex(regex, input), expected.map(|s| s.into()));
        };

        regex_test(r"foo=(\S+)", "foo=bar", Some("bar"));
        regex_test(r"foo=(\S+)", "bar=foo", None);
        regex_test(r"foo=(\S+)", "foo=bar baz", Some("bar"));
        regex_test(r"foo=(\S+)", "foo=", None);
    }

    #[test]
    fn exe_path() {
        #[cfg(unix)]
        assert!(is_executable_in_path("cargo"));

        #[cfg(windows)]
        assert!(is_executable_in_path("cargo.exe"));
    }

    #[test]
    fn globs() {
        let set = glob(["*.txt", "*.md"]).unwrap();
        assert!(set.is_match("README.md"));
        assert!(set.is_match("LICENSE.txt"));
    }

    #[ignore]
    #[test]
    fn sysinfo_test() {
        use sysinfo::{
            ProcessRefreshKind,
            RefreshKind,
            System,
        };

        let app_process_name = OsString::from(APP_PROCESS_NAME);
        let system = System::new_with_specifics(RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()));
        cfg_if! {
            if #[cfg(windows)] {
                let mut processes = system.processes_by_name(&app_process_name);
                assert!(processes.next().is_some());
            } else {
                let mut processes = system.processes_by_exact_name(&app_process_name);
                assert!(processes.next().is_some());
            }
        }
    }

    use std::cmp::Ordering;

    #[test]
    fn test_partitioned_compare() {
        assert_eq!(partitioned_compare("1.2.3", "1.2.3", '.'), Ordering::Equal);
        assert_eq!(partitioned_compare("1.2.3", "1.2.2", '.'), Ordering::Greater);
        assert_eq!(partitioned_compare("4-a-b", "4-a-c", '-'), Ordering::Less);
        assert_eq!(partitioned_compare("0?0?0", "0?0", '?'), Ordering::Greater);
    }

    #[test]
    fn test_gen_hex_string() {
        let hex = gen_hex_string();
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn test_current_exe_origin() {
        current_exe_origin().unwrap();
    }
}
