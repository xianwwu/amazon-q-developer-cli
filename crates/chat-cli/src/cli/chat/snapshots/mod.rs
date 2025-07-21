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

use crate::cli::ConversationState;
use crate::os::Os;

// ######## HARDCODED VALUES ########
const SHADOW_REPO_DIR: &str = "/Users/kiranbug/.aws/amazonq/shadow";
// ######## ---------------- ########

pub struct SnapshotManager {
    pub repo: Repository,
    pub modified_table: HashMap<PathBuf, u64>,
    pub snapshot_map: HashMap<Oid, Snapshot>,
    pub latest_snapshot: Oid,
    pub snapshot_count: u64,
}

pub struct Snapshot {
    pub oid: Oid,
    pub timestamp: u64,
    pub message: String,
    pub index: u64,
    pub modification: ModificationState,
}

pub struct ModificationState {
    pub creations: Vec<PathBuf>,
    pub deletions: Vec<PathBuf>,
    pub modifications: Vec<PathBuf>,
}

impl ModificationState {
    pub fn new() -> Self {
        Self {
            creations: Vec::new(),
            deletions: Vec::new(),
            modifications: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.creations.is_empty() && self.deletions.is_empty() && self.modifications.is_empty()
    }
}

impl SnapshotManager {
    pub async fn init(os: &Os) -> Result<Self> {
        let options = RepositoryInitOptions::new();
        let repo = Repository::init_opts(SHADOW_REPO_DIR, &options)?;

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
        }

        Ok(Self {
            repo,
            modified_table: HashMap::new(),
            snapshot_map: HashMap::new(),
            latest_snapshot: Oid::zero(),
            snapshot_count: 0,
        })
    }

    /// Checks if any files were modified since the last snapshot
    ///
    /// This is used as a fast check before we send any summarization request
    /// so user's don't have to wait if nothing was modified
    pub async fn any_modified(&self, os: &Os) -> Result<bool> {
        let cwd = os.env.current_dir()?;

        // Forward walk: checks for creations and modifications
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
            let cwd_path = entry.path();
            if cwd_path.is_dir() {
                continue;
            }

            let last_modified = match self.modified_table.get(cwd_path) {
                Some(time) => time,
                None => return Ok(true),
            };
            let new_modified = get_modified_timestamp(os, cwd_path).await?;
            if new_modified > *last_modified {
                return Ok(true);
            }
        }

        // Reverse walk: checks for deletions
        for entry in WalkDir::new(SHADOW_REPO_DIR)
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
            let shadow_path = entry.path();
            let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
            if shadow_path.is_dir() {
                continue;
            }

            if !os.fs.exists(cwd_path) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn stage_all_modified(&mut self, os: &Os) -> Result<ModificationState> {
        let mut index = self.repo.index()?;
        let cwd = os.env.current_dir()?;
        let mut modification = ModificationState::new();

        // FIX: switch reverse and foward walks

        // "Forward walk": stages all modifications and creations
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
            let cwd_path = entry.path();
            let shadow_path = convert_path(&cwd, cwd_path, SHADOW_REPO_DIR)?;
            let relative_path = cwd_path.strip_prefix(&cwd)?;

            // If path is directory, create and stage if needed
            if cwd_path.is_dir() {
                if !os.fs.exists(&shadow_path) {
                    copy_file_to_dir(os, &cwd, cwd_path, SHADOW_REPO_DIR).await?;

                    // Staging requires relative paths
                    index.add_path(relative_path)?;
                }
                continue;
            }

            let new_modified = get_modified_timestamp(os, cwd_path).await?;

            // Handles newly created files
            let last_modified = match self.modified_table.get(cwd_path) {
                Some(time) => time,
                None => &0,
            };

            // Update table and shadow repo if modified
            if new_modified > *last_modified {
                if os.fs.exists(shadow_path) {
                    modification.modifications.push(relative_path.to_path_buf());
                } else {
                    modification.creations.push(relative_path.to_path_buf());
                }
                self.modified_table.insert(cwd_path.to_path_buf(), new_modified);
                copy_file_to_dir(os, &cwd, cwd_path, SHADOW_REPO_DIR).await?;

                // Staging requires relative paths)
                index.add_path(relative_path)?;
            }
        }

        // "Reverse walk": stages all deletions
        for entry in WalkDir::new(SHADOW_REPO_DIR)
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
            let shadow_path = entry.path();
            let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
            let relative_path = shadow_path.strip_prefix(SHADOW_REPO_DIR)?;

            // If path is directory, delete and stage if needed
            if shadow_path.is_dir() {
                if !os.fs.exists(&cwd_path) {
                    os.fs.remove_dir_all(shadow_path).await?;

                    // Staging requires relative paths
                    index.add_path(relative_path)?;
                }
                continue;
            }

            // Update table and shadow repo if deleted
            // FIX: removing the entry is probably not the best choice
            if !os.fs.exists(&cwd_path) {
                modification.deletions.push(relative_path.to_path_buf());
                self.modified_table.remove(&shadow_path.to_path_buf());
                os.fs.remove_file(shadow_path).await?;

                // Staging requires relative paths
                index.remove_path(relative_path)?;
            }
        }
        index.write()?;
        Ok(modification)
    }

    pub async fn create_snapshot(&mut self, os: &Os, message: &str) -> Result<Option<Oid>> {
        let modification = match self.stage_all_modified(os).await {
            Ok(m) if !m.is_empty() => m,
            Ok(_) => return Ok(None),
            Err(_) => bail!("Could not stage changes"),
        };

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
            index: self.snapshot_count,
            modification,
        });
        self.latest_snapshot = oid;
        self.snapshot_count += 1;

        Ok(Some(oid))
    }

    pub async fn restore(&mut self, os: &Os, conversation: &mut ConversationState, commit_hash: &str) -> Result<Oid> {
        let snapshot = match self.snapshot_map.get(&Oid::from_str(commit_hash)?) {
            Some(s) => s,
            None => bail!("Commit not found in map"),
        };
        let restore_index = snapshot.index;
        let cwd = os.env.current_dir()?;

        let oid = self.reset_hard(commit_hash).await?;

        // FIX: switch reverse and foward walks

        // "Forward walk": restores all modifications and creations
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
            let shadow_path = entry.path();
            let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;

            // If path is directory, create and stage if needed
            if shadow_path.is_dir() {
                if !os.fs.exists(cwd_path) {
                    copy_file_to_dir(os, SHADOW_REPO_DIR, shadow_path, &cwd).await?;
                }
                continue;
            }

            // FIX: update modification table when snapshot is restored

            copy_file_to_dir(os, SHADOW_REPO_DIR, shadow_path, &cwd).await?;
        }

        // "Reverse walk": restores all deletions
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
            let cwd_path = entry.path();
            let shadow_path = convert_path(&cwd, cwd_path, SHADOW_REPO_DIR)?;

            // If path is directory, delete if needed
            if cwd_path.is_dir() {
                if !os.fs.exists(&shadow_path) {
                    os.fs.remove_dir_all(cwd_path).await?;
                }
                continue;
            }

            // Update cwd if file doesn't exist in shadow
            // FIX: removing the entry is probably not the best choice
            if !os.fs.exists(&shadow_path) {
                os.fs.remove_file(cwd_path).await?;
            }
        }

        for _ in restore_index..self.snapshot_count {
            conversation.pop_from_history();
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

    pub async fn clean(os: &Os) -> Result<()> {
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
    let path = path.as_ref();
    let target_path = convert_path(prefix, &path, destination)?;
    if path.is_dir() && !os.fs.exists(&target_path) {
        os.fs.create_dir_all(target_path).await?;
    } else if path.is_file() {
        os.fs.copy(path, target_path).await?;
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
