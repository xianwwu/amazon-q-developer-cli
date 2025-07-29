use std::io::Write;
use std::path::{
    Path,
    PathBuf,
};

use crossterm::queue;
use crossterm::style::{
    self,
    Color,
    Stylize,
};
use eyre::{
    Result,
    bail,
};
use globset::{
    Glob,
    GlobSetBuilder,
};

use serde::Deserialize;
use tracing::{
    error,
    warn,
};

use super::{
    InvokeOutput,
    format_path,
    sanitize_path_tool_arg,
};
use crate::cli::agent::{
    Agent,
    PermissionEvalResult,
};
use crate::cli::chat::checkpoint::{
    CheckpointManager,
    collect_paths_and_data,
};
use crate::os::Os;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "command")]
pub enum FsRemove {
    #[serde(rename = "remove_file")]
    RemoveFile { path: String, summary: Option<String> },
    #[serde(rename = "remove_dir")]
    RemoveDir { path: String, summary: Option<String> },
}

impl FsRemove {
    pub async fn invoke(&self, os: &Os, output: &mut impl Write) -> Result<InvokeOutput> {
        let cwd = os.env.current_dir()?;
        // ########## CHECKPOINTING ##########
        //
        // To handle both files and directories, we first collect all of our paths and data
        let (paths, datas) = self.gather_paths_for_checkpointing(os).await?;
        if let Ok(manager) = &mut CheckpointManager::load_manager(os).await {
            // Save all data
            manager.checkpoint_with_data(os, paths.clone(), datas.clone()).await?;

            // Track files as deleted
            manager
                .checkpoint_with_data(os, paths.clone(), vec![None; paths.len()])
                .await?;

            CheckpointManager::save_manager(os, &manager).await?;
        }
        // ########## /CHECKPOINTING ##########

        match self {
            FsRemove::RemoveFile { path, .. } => {
                let path = sanitize_path_tool_arg(os, path);
                queue!(
                    output,
                    style::Print("Removing: "),
                    style::SetForegroundColor(Color::Green),
                    style::Print(format_path(cwd, &path)),
                    style::ResetColor,
                )?;

                remove_file(os, &path).await?;
                Ok(Default::default())
            },
            FsRemove::RemoveDir { path, .. } => {
                let path = sanitize_path_tool_arg(os, path);
                queue!(
                    output,
                    style::Print("Removing: "),
                    style::SetForegroundColor(Color::Green),
                    style::Print(format_path(cwd, &path)),
                    style::ResetColor,
                )?;
                remove_dir(os, path).await?;
                Ok(Default::default())
            },
        }
    }

    pub async fn gather_paths_for_checkpointing(&self, os: &Os) -> Result<(Vec<PathBuf>, Vec<Option<Vec<u8>>>)> {
        let cwd = os.env.current_dir()?;
        match self {
            Self::RemoveFile { path, .. } => {
                let canonical = cwd.join(sanitize_path_tool_arg(os, path));
                Ok((vec![canonical.clone()], vec![Some(os.fs.read(canonical).await?)]))
            },
            Self::RemoveDir { path, .. } => {
                let dir = cwd.join(sanitize_path_tool_arg(os, path));

                let (relative_paths, datas) = collect_paths_and_data(os, path).await?;
                let paths: Vec<PathBuf> = relative_paths.iter().map(|p| dir.join(p)).collect();
                let datas: Vec<Option<Vec<u8>>> = datas.into_iter().map(Some).collect();

                Ok((paths, datas))
            },
        }
    }

    pub async fn queue_description(&self, os: &Os, output: &mut impl Write) -> Result<()> {
        self.print_relative_path_to_remove(os, output)?;
        match self {
            FsRemove::RemoveFile { .. } => {
                // if let Some(summary) = self.get_content_summary() {
                //     queue!(
                //         output,
                //         style::Print("Content summary: ".green()),
                //         style::Print(summary),
                //         style::Print("\n"),
                //     )?;
                // }

                // Display summary as purpose if available
                super::display_purpose(self.get_summary(), output)?;

                Ok(())
            },
            FsRemove::RemoveDir { path, .. } => {
                self.print_all_directory_entries(os, &path, output).await?;

                // Display summary as purpose if available
                super::display_purpose(self.get_summary(), output)?;
                Ok(())
            },
        }
    }

    pub async fn validate(&mut self, os: &Os) -> Result<()> {
        match self {
            FsRemove::RemoveFile { path, .. } => {
                if path.is_empty() {
                    bail!("Path must not be empty");
                };
                let path = sanitize_path_tool_arg(os, &path);
                if !path.exists() {
                    bail!("File {} does not exist", path.display());
                }
                if !path.is_file() {
                    bail!("Path {} is not a file", path.display());
                }
            },
            FsRemove::RemoveDir { path, .. } => {
                if path.is_empty() {
                    bail!("Path must not be empty");
                };
                let path = sanitize_path_tool_arg(os, &path);
                if !path.exists() {
                    bail!("Directory {} does not exist", path.display());
                }
                if !path.is_dir() {
                    bail!("Path {} is not a directory", path.display());
                }
            },
        }
        Ok(())
    }

    fn print_relative_path_to_remove(&self, os: &Os, output: &mut impl Write) -> Result<()> {
        let cwd = os.env.current_dir()?;
        let path = match self {
            FsRemove::RemoveFile { path, .. } => path,
            FsRemove::RemoveDir { path, .. } => path,
        };
        // Sanitize the path to handle tilde expansion
        let path = sanitize_path_tool_arg(os, path);
        let relative_path = format_path(cwd, &path);
        queue!(
            output,
            style::Print(" Removing: "),
            style::SetForegroundColor(Color::Green),
            style::Print(&relative_path),
            style::ResetColor,
            style::Print("\n\n"),
        )?;
        Ok(())
    }

    /// Returns the summary from any variant
    pub fn get_summary(&self) -> Option<&String> {
        match self {
            FsRemove::RemoveFile { summary, .. } => summary.as_ref(),
            FsRemove::RemoveDir { summary, .. } => summary.as_ref(),
        }
    }

    async fn print_all_directory_entries(
        &self,
        os: &Os,
        path: impl AsRef<Path>,
        output: &mut impl Write,
    ) -> Result<()> {
        const PATHS_BEFORE_TRUNCATION: usize = 5;

        let cwd = os.env.current_dir()?;
        let mut entries = os.fs.read_dir(path).await?;
        let mut count = 0;

        while let Some(entry) = entries.next_entry().await? {
            if count < PATHS_BEFORE_TRUNCATION {
                let path = sanitize_path_tool_arg(os, entry.path());
                let relative_path = format_path(&cwd, &path);
                queue!(
                    output,
                    style::Print(" - ".red()),
                    style::Print(format!("./{}", relative_path).dark_grey()),
                )?;
                if count < PATHS_BEFORE_TRUNCATION - 1 {
                    queue!(output, style::Print("\n"))?;
                }
            }
            count += 1;
        }
        if count >= PATHS_BEFORE_TRUNCATION {
            queue!(
                output,
                style::Print(format!(" ...and {} more", count - PATHS_BEFORE_TRUNCATION).dark_grey()),
                style::Print("\n")
            )?;
        }
        Ok(())
    }

    pub fn eval_perm(&self, agent: &Agent) -> PermissionEvalResult {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Settings {
            #[serde(default)]
            allowed_paths: Vec<String>,
            #[serde(default)]
            denied_paths: Vec<String>,
        }

        let is_in_allowlist = agent.allowed_tools.contains("fs_remove");
        match agent.tools_settings.get("fs_remove") {
            Some(settings) if is_in_allowlist => {
                let Settings {
                    allowed_paths,
                    denied_paths,
                } = match serde_json::from_value::<Settings>(settings.clone()) {
                    Ok(settings) => settings,
                    Err(e) => {
                        error!("Failed to deserialize tool settings for fs_remove: {:?}", e);
                        return PermissionEvalResult::Ask;
                    },
                };
                let allow_set = {
                    let mut builder = GlobSetBuilder::new();
                    for path in &allowed_paths {
                        if let Ok(glob) = Glob::new(path) {
                            builder.add(glob);
                        } else {
                            warn!("Failed to create glob from path given: {path}. Ignoring.");
                        }
                    }
                    builder.build()
                };

                let deny_set = {
                    let mut builder = GlobSetBuilder::new();
                    for path in &denied_paths {
                        if let Ok(glob) = Glob::new(path) {
                            builder.add(glob);
                        } else {
                            warn!("Failed to create glob from path given: {path}. Ignoring.");
                        }
                    }
                    builder.build()
                };

                match (allow_set, deny_set) {
                    (Ok(allow_set), Ok(deny_set)) => {
                        match self {
                            Self::RemoveFile { path, .. } | Self::RemoveDir { path, .. } => {
                                if deny_set.is_match(path) {
                                    return PermissionEvalResult::Deny;
                                }
                                if allow_set.is_match(path) {
                                    return PermissionEvalResult::Allow;
                                }
                            },
                        }
                        PermissionEvalResult::Ask
                    },
                    (allow_res, deny_res) => {
                        if let Err(e) = allow_res {
                            warn!("fs_remove failed to build allow set: {:?}", e);
                        }
                        if let Err(e) = deny_res {
                            warn!("fs_remove failed to build deny set: {:?}", e);
                        }
                        warn!("One or more detailed args failed to parse, falling back to ask");
                        PermissionEvalResult::Ask
                    },
                }
            },
            None if is_in_allowlist => PermissionEvalResult::Allow,
            _ => PermissionEvalResult::Ask,
        }
    }
}

/// Removes file at `path`
async fn remove_file(os: &Os, path: impl AsRef<Path>) -> Result<()> {
    let path_ref = path.as_ref();
    tracing::debug!("Removing file: {:?}", path_ref);
    os.fs.remove_file(path_ref).await?;
    Ok(())
}

/// Removes file at `path`
async fn remove_dir(os: &Os, path: impl AsRef<Path>) -> Result<()> {
    let path_ref = path.as_ref();
    tracing::debug!("Removing directory: {:?}", path_ref);
    os.fs.remove_dir_all(path_ref).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::chat::util::test::setup_test_directory;

    #[test]
    fn test_fs_remove_deserialize() {
        let path = "/my-file";

        // file
        let v = serde_json::json!({
            "path": path,
            "command": "remove_file",
        });
        let fw = serde_json::from_value::<FsRemove>(v).unwrap();
        assert!(matches!(fw, FsRemove::RemoveFile { .. }));

        // dir
        let v = serde_json::json!({
            "path": path,
            "command": "remove_dir",
        });
        let fw = serde_json::from_value::<FsRemove>(v).unwrap();
        assert!(matches!(fw, FsRemove::RemoveDir { .. }));
    }

    #[test]
    fn test_fs_remove_deserialize_with_summary() {
        let path = "/my-file";
        let summary = "Added hello world content";

        // file with summary
        let v = serde_json::json!({
            "path": path,
            "command": "remove_file",
            "summary": summary
        });
        let fw = serde_json::from_value::<FsRemove>(v).unwrap();
        assert!(matches!(fw, FsRemove::RemoveFile { .. }));
        if let FsRemove::RemoveFile { summary: s, .. } = &fw {
            assert_eq!(s.as_ref().unwrap(), summary);
        };

        // dir with summary
        let v = serde_json::json!({
            "path": path,
            "command": "remove_dir",
            "summary": summary
        });
        let fw = serde_json::from_value::<FsRemove>(v).unwrap();
        assert!(matches!(fw, FsRemove::RemoveDir { .. }));
        if let FsRemove::RemoveDir { summary: s, .. } = &fw {
            assert_eq!(s.as_ref().unwrap(), summary);
        };
    }

    #[tokio::test]
    async fn test_fs_remove_tool_remove_file() {
        let os = setup_test_directory().await;
        let mut stdout = std::io::stdout();

        let path = "/test-file";
        os.fs.create_new(path).await.unwrap();
        let v = serde_json::json!({
            "path": path,
            "command": "remove_file",
        });
        serde_json::from_value::<FsRemove>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        assert!(!os.fs.exists(path));
    }

    #[tokio::test]
    async fn test_fs_remove_tool_remove_dir() {
        let test_dir_path1 = "/test_dir1";
        let test_dir_path2 = "/test_dir2";
        let test_dir_path3 = "/test_dir3/another_dir/whoa_another/no_way_another";

        // Single empty directory
        let os = setup_test_directory().await;
        let mut stdout = std::io::stdout();

        os.fs.create_dir(test_dir_path1).await.unwrap();
        let v = serde_json::json!({
            "path": test_dir_path1,
            "command": "remove_dir",
        });
        serde_json::from_value::<FsRemove>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        assert!(!os.fs.exists(test_dir_path1));

        // Populated directory
        os.fs.create_dir(test_dir_path2).await.unwrap();
        for i in 0..10 {
            os.fs
                .create_new(PathBuf::from(test_dir_path2).join(format!("file{}", i)))
                .await
                .unwrap();
        }

        let v = serde_json::json!({
            "path": test_dir_path2,
            "command": "remove_dir",
        });
        serde_json::from_value::<FsRemove>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        // Check if all files were deleted
        assert!(!os.fs.exists(test_dir_path2));
        for i in 0..10 {
            assert!(!os.fs.exists(PathBuf::from(test_dir_path2).join(format!("file{}", i))));
        }

        // Nested directories
        os.fs.create_dir_all(test_dir_path3).await.unwrap();
        let v = serde_json::json!({
            "path": "/test_dir3/",
            "command": "remove_dir",
        });
        serde_json::from_value::<FsRemove>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        // Check if all directories were deleted
        for ancestor in PathBuf::from(test_dir_path3)
            .ancestors()
            .take_while(|&p| p != Path::new("/"))
        {
            assert!(!os.fs.exists(ancestor));
        }
    }

    #[tokio::test]
    async fn test_fs_remove_with_tilde_paths() {
        // Create a test context
        let os = Os::new().await.unwrap();
        let mut stdout = std::io::stdout();

        // Get the home directory from the context
        let home_dir = os.env.home().unwrap_or_default();
        println!("Test home directory: {:?}", home_dir);

        // Create a file directly in the home directory first to ensure it exists
        let home_path = os.fs.chroot_path(&home_dir);
        println!("Chrooted home path: {:?}", home_path);

        // Ensure the home directory exists
        os.fs.create_dir_all(&home_path).await.unwrap();

        // Create test file
        os.fs.create_new(home_path.join("test_file.jpg")).await.unwrap();

        let v = serde_json::json!({
            "path": "~/test_file.jpg",
            "command": "remove_file",
        });

        let result = serde_json::from_value::<FsRemove>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await;

        match &result {
            Ok(_) => println!("Removing ~/file.txt succeeded"),
            Err(e) => println!("Removing ~/file.txt failed: {:?}", e),
        }

        assert!(result.is_ok(), "Writing to ~/file.txt should succeed");
        assert!(!os.fs.exists(home_path.join("test_file.jpg")));
    }
}
