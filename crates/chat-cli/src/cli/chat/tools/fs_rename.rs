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
pub struct FsRename {
    original_path: String,
    new_path: String,
}

impl FsRename {
    pub async fn invoke(&self, os: &Os, output: &mut impl Write) -> Result<InvokeOutput> {
        self.print_relative_paths_from_and_to(os, output)?;
        rename_path(
            os,
            sanitize_path_tool_arg(os, &self.original_path),
            sanitize_path_tool_arg(os, &self.new_path),
        )
        .await?;
        
        // ########## CHECKPOINTING ##########
        let (original_paths, new_paths, datas) = self.gather_paths_for_checkpointing(os).await?;

        if let Ok(manager) = &mut CheckpointManager::load_manager(os).await {
            // Save old paths and data
            manager
                .checkpoint_with_data(os, original_paths.clone(), datas.clone())
                .await?;

            // Track both original and new paths as deleted (necessary intermediate step)
            manager
                .checkpoint_with_data(
                    os,
                    [original_paths.clone(), new_paths.clone()].concat(),
                    vec![None; original_paths.len() + new_paths.len()],
                )
                .await?;

            // Track new paths and their contents
            manager.checkpoint_with_data(os, new_paths, datas).await?;

            CheckpointManager::save_manager(os, &manager).await?;
        }
        // ########## /CHECKPOINTING ##########

        Ok(Default::default())
    }

    pub async fn gather_paths_for_checkpointing(&self, os: &Os) -> Result<(Vec<PathBuf>, Vec<PathBuf>, Vec<Option<Vec<u8>>>)> {
        let cwd = os.env.current_dir()?;
        if PathBuf::from(&self.new_path).is_file() {
            let original_canonical = cwd.join(sanitize_path_tool_arg(os, &self.original_path));
            let new_canonical = cwd.join(sanitize_path_tool_arg(os, &self.new_path));

            Ok((vec![original_canonical], vec![new_canonical], vec![Some(
                os.fs.read(&self.new_path).await?,
            )]))
        } else {
            let original_dir = cwd.join(sanitize_path_tool_arg(os, &self.original_path));
            let new_dir = cwd.join(sanitize_path_tool_arg(os, &self.new_path));

            let (relative_paths, datas) = collect_paths_and_data(os, &new_dir).await?;
            let original_paths: Vec<PathBuf> = relative_paths.iter().map(|p| original_dir.join(p)).collect();
            let new_paths: Vec<PathBuf> = relative_paths.iter().map(|p| new_dir.join(p)).collect();
            let datas: Vec<Option<Vec<u8>>> = datas.into_iter().map(Some).collect();

            Ok((original_paths, new_paths, datas))
        }
    }

    pub fn queue_description(&self, os: &Os, output: &mut impl Write) -> Result<()> {
        self.print_relative_paths_from_and_to(os, output)?;
        if os.fs.exists(&self.new_path) {
            queue!(
                output,
                style::Print(format!(" (warning: {} already exists and will be overwritten)", self.new_path).red()),
            )?;
        }
        queue!(output, style::Print("\n"))?;
        Ok(())
    }

    pub fn get_summary(&self, os: &Os) -> Result<String> {
        let cwd = os.env.current_dir()?;
        let (from, to) = (
            format_path(&cwd, sanitize_path_tool_arg(os, &self.original_path)),
            format_path(&cwd, sanitize_path_tool_arg(os, &self.new_path)),
        );
        Ok(format!("{} to {}", from, to))
    }

    pub async fn validate(&mut self, os: &Os) -> Result<()> {
        if self.original_path.is_empty() | self.new_path.is_empty() {
            bail!("Path must not be empty");
        };
        let original_path = sanitize_path_tool_arg(os, &self.original_path);
        if !original_path.exists() {
            bail!("File {} does not exist", original_path.display());
        }
        Ok(())
    }

    fn print_relative_paths_from_and_to(&self, os: &Os, output: &mut impl Write) -> Result<()> {
        let summary = self.get_summary(os)?;
        queue!(
            output,
            style::Print("Renaming: "),
            style::SetForegroundColor(Color::Green),
            style::Print(summary),
            style::ResetColor,
        )?;
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

        let is_in_allowlist = agent.allowed_tools.contains("fs_rename");
        match agent.tools_settings.get("fs_rename") {
            Some(settings) if is_in_allowlist => {
                let Settings {
                    allowed_paths,
                    denied_paths,
                } = match serde_json::from_value::<Settings>(settings.clone()) {
                    Ok(settings) => settings,
                    Err(e) => {
                        error!("Failed to deserialize tool settings for fs_rename: {:?}", e);
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
                        if deny_set.is_match(&self.original_path) | deny_set.is_match(&self.new_path) {
                            return PermissionEvalResult::Deny;
                        }
                        if allow_set.is_match(&self.original_path) && allow_set.is_match(&self.new_path) {
                            return PermissionEvalResult::Allow;
                        }
                        PermissionEvalResult::Ask
                    },
                    (allow_res, deny_res) => {
                        if let Err(e) = allow_res {
                            warn!("fs_rename failed to build allow set: {:?}", e);
                        }
                        if let Err(e) = deny_res {
                            warn!("fs_rename failed to build deny set: {:?}", e);
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

/// Renames path from "from" to "to"
async fn rename_path(os: &Os, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    tracing::debug!("Renaming path from {:?} to {:?}", from, to);
    os.fs.rename(from, to).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::chat::util::test::setup_test_directory;
    use crate::os::Os;

    #[test]
    fn test_fs_rename_deserialize() {
        let original_path = "/my-file";
        let new_path = "/my-renamed-file";

        let v = serde_json::json!({
            "original_path": original_path,
            "new_path": new_path,
        });
        let fr = serde_json::from_value::<FsRename>(v).unwrap();
        assert_eq!(fr.original_path, original_path);
        assert_eq!(fr.new_path, new_path);
    }

    #[tokio::test]
    async fn test_fs_rename_tool_rename_file() {
        let os = setup_test_directory().await;
        let mut stdout = std::io::stdout();

        let original_path = "/test-file";
        let new_path = "/renamed-test-file";

        // Create the original file
        os.fs.create_new(original_path).await.unwrap();
        assert!(os.fs.exists(original_path));

        let v = serde_json::json!({
            "original_path": original_path,
            "new_path": new_path,
        });
        serde_json::from_value::<FsRename>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        // Check that original file no longer exists and new file exists
        assert!(!os.fs.exists(original_path));
        assert!(os.fs.exists(new_path));
    }

    #[tokio::test]
    async fn test_fs_rename_tool_rename_directory() {
        let os = setup_test_directory().await;
        let mut stdout = std::io::stdout();

        let original_dir = "/test-dir";
        let new_dir = "/renamed-test-dir";

        // Create the original directory with some files
        os.fs.create_dir(original_dir).await.unwrap();
        os.fs
            .create_new(PathBuf::from(original_dir).join("file1.txt"))
            .await
            .unwrap();
        os.fs
            .create_new(PathBuf::from(original_dir).join("file2.txt"))
            .await
            .unwrap();

        assert!(os.fs.exists(original_dir));
        assert!(os.fs.exists(PathBuf::from(original_dir).join("file1.txt")));

        let v = serde_json::json!({
            "original_path": original_dir,
            "new_path": new_dir,
        });
        serde_json::from_value::<FsRename>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        // Check that original directory no longer exists and new directory exists with files
        assert!(!os.fs.exists(original_dir));
        assert!(os.fs.exists(new_dir));
        assert!(os.fs.exists(PathBuf::from(new_dir).join("file1.txt")));
        assert!(os.fs.exists(PathBuf::from(new_dir).join("file2.txt")));
    }

    #[tokio::test]
    async fn test_fs_rename_tool_rename_to_subdirectory() {
        let os = setup_test_directory().await;
        let mut stdout = std::io::stdout();

        let original_path = "/test-file.txt";
        let target_dir = "/target-dir";
        let new_path = "/target-dir/moved-file.txt";

        // Create the original file and target directory
        os.fs.create_new(original_path).await.unwrap();
        os.fs.create_dir(target_dir).await.unwrap();

        assert!(os.fs.exists(original_path));
        assert!(os.fs.exists(target_dir));

        let v = serde_json::json!({
            "original_path": original_path,
            "new_path": new_path,
        });
        serde_json::from_value::<FsRename>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        // Check that file was moved to the subdirectory
        assert!(!os.fs.exists(original_path));
        assert!(os.fs.exists(new_path));
    }

    #[tokio::test]
    async fn test_fs_rename_tool_rename_nested_directories() {
        let os = setup_test_directory().await;
        let mut stdout = std::io::stdout();

        let original_nested_dir = "/parent/child/grandchild";
        let new_nested_dir = "/parent/renamed-child/grandchild";

        // Create nested directory structure
        os.fs.create_dir_all(original_nested_dir).await.unwrap();
        os.fs
            .create_new(PathBuf::from(original_nested_dir).join("nested-file.txt"))
            .await
            .unwrap();

        assert!(os.fs.exists(original_nested_dir));
        assert!(os.fs.exists(PathBuf::from(original_nested_dir).join("nested-file.txt")));

        let v = serde_json::json!({
            "original_path": "/parent/child",
            "new_path": "/parent/renamed-child",
        });
        serde_json::from_value::<FsRename>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await
            .unwrap();

        // Check that nested structure was renamed correctly
        assert!(!os.fs.exists("/parent/child"));
        assert!(os.fs.exists("/parent/renamed-child"));
        assert!(os.fs.exists(new_nested_dir));
        assert!(os.fs.exists(PathBuf::from(new_nested_dir).join("nested-file.txt")));
    }

    #[tokio::test]
    async fn test_fs_rename_with_tilde_paths() {
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
        os.fs.create_new(home_path.join("test_file.txt")).await.unwrap();

        let v = serde_json::json!({
            "original_path": "~/test_file.txt",
            "new_path": "~/renamed_file.txt",
        });

        let result = serde_json::from_value::<FsRename>(v)
            .unwrap()
            .invoke(&os, &mut stdout)
            .await;

        match &result {
            Ok(_) => println!("Renaming ~/test_file.txt to ~/renamed_file.txt succeeded"),
            Err(e) => println!("Renaming ~/test_file.txt to ~/renamed_file.txt failed: {:?}", e),
        }

        assert!(
            result.is_ok(),
            "Renaming ~/test_file.txt to ~/renamed_file.txt should succeed"
        );
        assert!(!os.fs.exists(home_path.join("test_file.txt")));
        assert!(os.fs.exists(home_path.join("renamed_file.txt")));
    }

    #[tokio::test]
    async fn test_fs_rename_validation_empty_paths() {
        let os = setup_test_directory().await;

        // Test empty original_path
        let mut fs_rename = FsRename {
            original_path: "".to_string(),
            new_path: "/valid-path".to_string(),
        };
        let result = fs_rename.validate(&os).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Path must not be empty"));

        // Test empty new_path
        let mut fs_rename = FsRename {
            original_path: "/valid-path".to_string(),
            new_path: "".to_string(),
        };
        let result = fs_rename.validate(&os).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Path must not be empty"));

        // Test both empty
        let mut fs_rename = FsRename {
            original_path: "".to_string(),
            new_path: "".to_string(),
        };
        let result = fs_rename.validate(&os).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Path must not be empty"));
    }

    #[tokio::test]
    async fn test_fs_rename_validation_nonexistent_file() {
        let os = setup_test_directory().await;

        let mut fs_rename = FsRename {
            original_path: "/nonexistent-file".to_string(),
            new_path: "/new-path".to_string(),
        };
        let result = fs_rename.validate(&os).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_fs_rename_validation_success() {
        let os = setup_test_directory().await;

        // Create a file to rename
        let original_path = "/existing-file";
        os.fs.create_new(original_path).await.unwrap();

        let mut fs_rename = FsRename {
            original_path: original_path.to_string(),
            new_path: "/new-path".to_string(),
        };
        let result = fs_rename.validate(&os).await;
        assert!(result.is_ok());
    }
}
