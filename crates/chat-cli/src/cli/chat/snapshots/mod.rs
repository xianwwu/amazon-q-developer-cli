use std::collections::HashMap;
use std::path::{
    Path,
    PathBuf,
};
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use eyre::{
    Result,
    bail,
};
use git2::{
    ObjectType,
    Oid,
    Repository,
    RepositoryInitOptions,
    ResetType,
    Signature,
};
use walkdir::WalkDir;

use crate::os::Os;

// ######## HARDCODED VALUES ########
const SHADOW_REPO_DIR: &str = "/Users/kiranbug/.aws/amazonq/shadow";
// ######## ---------------- ########

pub struct SnapshotManager {
    pub repo: Repository,
    pub modified_table: HashMap<PathBuf, u64>,
    pub snapshot_map: HashMap<Oid, Snapshot>,
}

pub struct Snapshot {
    pub oid: Oid,
    pub timestamp: u64,
    pub message: String,
    pub reason: Option<String>,
}

impl SnapshotManager {
    pub async fn init(os: &Os) -> Result<Self> {
        let options = RepositoryInitOptions::new();
        let repo = Repository::init_opts(SHADOW_REPO_DIR, &options)?;
        let mut modified_table: HashMap<PathBuf, u64> = HashMap::new();

        let cwd = os.env.current_dir()?;

        // Copy everything into the shadow repo
        for entry in WalkDir::new(&cwd)
            .into_iter()
            .filter_entry(|e| !path_contains(e.path(), ".git"))
            .skip(1)
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    // FIX: what do we do here instead of silently failing?
                    continue;
                },
            };
            let path = entry.path();
            copy_file_to_dir(os, &cwd, path, SHADOW_REPO_DIR).await?;
            if path.is_file() {
                modified_table.insert(path.to_path_buf(), get_modified_timestamp(os, path).await?);
            }
        }

        Ok(Self {
            repo,
            modified_table: HashMap::new(),
            snapshot_map: HashMap::new(),
        })
    }

    async fn stage_all_modified(&mut self, os: &Os) -> Result<()> {
        let mut index = self.repo.index()?;
        let cwd = os.env.current_dir()?;
        for entry in WalkDir::new(&cwd)
            .into_iter()
            .filter_entry(|e| !path_contains(e.path(), ".git"))
            .skip(1)
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    // FIX: SILENT FAIL
                    continue;
                },
            };
            let path = entry.path();

            // If path is directory, create and stage if needed
            if path.is_dir() {
                if !os.fs.exists(convert_path(&cwd, path, SHADOW_REPO_DIR)?) {
                    copy_file_to_dir(os, &cwd, path, SHADOW_REPO_DIR).await?;

                    // Staging requires relative paths
                    index.add_path(path.strip_prefix(&cwd)?)?;
                }
                continue;
            }

            let new_modified = get_modified_timestamp(os, path).await?;

            // Handles newly created files
            let last_modified = match self.modified_table.get(path) {
                Some(time) => time,
                None => &0,
            };

            // Update table and shadow repo if modified
            if new_modified > *last_modified {
                self.modified_table.insert(path.to_path_buf(), new_modified);
                copy_file_to_dir(os, &cwd, path, SHADOW_REPO_DIR).await?;

                // Staging requires relative paths)
                index.add_path(path.strip_prefix(&cwd)?)?;
            }
        }
        index.write()?;
        Ok(())
    }

    pub async fn create_snapshot(&mut self, os: &Os, message: &str, reason: Option<&str>) -> Result<Oid> {
        self.stage_all_modified(os).await?;
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let signature = Signature::now("Q", "example@amazon.com")?;

        let parents = match self.repo.head() {
            Ok(head) => vec![head.peel_to_commit()?],
            Err(_) => Vec::new(),
        };

        let oid = self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents.iter().map(|c| c).collect::<Vec<_>>(),
        )?;

        // FIX: potentially unsafe conversion from i64 to u64?
        // Shouldn't be unsafe because it's time, but why did they use i64
        self.snapshot_map.insert(oid, Snapshot {
            oid,
            timestamp: signature.when().seconds() as u64,
            message: message.to_string(),
            reason: reason.map(String::from),
        });
        Ok(oid)
    }

    pub async fn restore(&mut self, os: &Os, commit_hash: &str) -> Result<Oid> {
        let oid = self.reset_hard(commit_hash).await?;
        for entry in WalkDir::new(SHADOW_REPO_DIR)
            .into_iter()
            .filter_entry(|e| !path_contains(e.path(), ".git"))
            .skip(1)
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    // FIX: what do we do here instead of silently failing?
                    continue;
                },
            };
            let path = entry.path();
            copy_file_to_dir(os, SHADOW_REPO_DIR, path, os.env.current_dir()?).await?;
        }
        Ok(oid)
    }

    async fn reset_hard(&mut self, commit_hash: &str) -> Result<Oid> {
        let obj = self.repo.revparse_single(commit_hash)?;
        let commit = obj.peel(ObjectType::Commit)?;
        let commit_id = commit.id();

        self.repo.reset(&commit, ResetType::Hard, None)?;
        Ok(commit_id)
    }

    pub async fn clean(&self, os: &Os) -> Result<()> {
        os.fs.remove_dir_all(SHADOW_REPO_DIR).await?;
        Ok(())
    }
}

pub fn get_timestamp() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    format!("{timestamp}")
}

pub async fn get_modified_timestamp(os: &Os, path: impl AsRef<Path>) -> Result<u64> {
    let file = os.fs.open(path).await?;
    Ok(file
        .metadata()
        .await?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

async fn copy_file_to_dir(
    os: &Os,
    prefix: impl AsRef<Path>,
    path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<()> {
    let target_path = convert_path(prefix, &path, destination)?;
    if path.as_ref().is_dir() && !os.fs.exists(&target_path) {
        os.fs.create_dir_all(target_path).await?;
    } else if path.as_ref().is_file() {
        os.fs.copy(path.as_ref(), target_path).await?;
    }
    Ok(())
}

fn convert_path(prefix: impl AsRef<Path>, path: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<PathBuf> {
    let relative_path = path.as_ref().strip_prefix(prefix)?;
    Ok(Path::join(destination.as_ref(), relative_path))
}

/// Returns true if path contains part
///
/// Generated by Q
fn path_contains(path: &Path, part: &str) -> bool {
    path.components().any(|component| {
        if let std::path::Component::Normal(name) = component {
            name == part
        } else {
            false
        }
    })
}
