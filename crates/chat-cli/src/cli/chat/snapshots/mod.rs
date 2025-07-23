use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{
    Path,
    PathBuf,
};
use std::sync::Arc;

use dircmp::Comparison;
use eyre::{
    Result,
    bail,
    eyre,
};
use git2::{
    ObjectType,
    Oid,
    Repository,
    RepositoryInitOptions,
    ResetType,
    Signature,
};
use regex::RegexSet;
use crate::cli::ConversationState;
use crate::os::Os;

// ######## HARDCODED VALUES ########
const SHADOW_REPO_DIR: &str = "/Users/kiranbug/.aws/amazonq/shadow";
// ######## ---------------- ########

#[derive(Clone)]
pub struct SnapshotManager {
    repo: Arc<Repository>,
    pub snapshot_count: usize,
    pub snapshot_table: Vec<Snapshot>,
   
    // Separated from snapshot table to ensure that oids 
    // are never used at a user-facing level
    oid_table: Vec<Oid>,

    // Contains modification timestamps for absolute paths in cwd
    pub modified_map: HashMap<PathBuf, u64>,

}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub oid: Oid,
    pub timestamp: u64,
    pub message: String,

    // For managing history undoing
    pub messages_since: usize,
}

impl SnapshotManager {
    pub fn init() -> Result<Self> {
        let options = RepositoryInitOptions::new();
        let repo = Repository::init_opts(SHADOW_REPO_DIR, &options)?;

        Ok(Self {
            repo: Arc::new(repo),
            snapshot_count: 0,
            snapshot_table: Vec::new(),
            oid_table: Vec::new(),
            modified_map: HashMap::new(),
        })
    }

    /// Checks if any files were modified since the last snapshot
    ///
    /// This is used as a fast check before we send any summarization request
    /// so users don't have to wait if nothing was modified
    pub async fn any_modified(&self, os: &Os) -> Result<bool> {
        let ignores = RegexSet::new(&[r".git"])?;
        let comparison = Comparison::new(ignores);

        let res = comparison.compare(SHADOW_REPO_DIR, os.env.current_dir()?.to_str().unwrap())?;
        Ok(!(res.changed.is_empty() && res.missing_left.is_empty() && res.missing_right.is_empty()))
    }

    async fn stage_all_modified(&mut self, os: &Os) -> Result<()> {
        let mut index = self.repo.index()?;
        let cwd = os.env.current_dir()?;

        let ignores = RegexSet::new(&[r".git"])?;
        let comparison = Comparison::new(ignores);
        let res = comparison.compare(SHADOW_REPO_DIR, os.env.current_dir()?.to_str().unwrap())?;

        // Handle modified files
        for shadow_path in res.changed.iter() {
            if shadow_path.is_file() {
                let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
                self.modified_map.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, &cwd_path).await?);
                copy_file_to_dir(os, &cwd, cwd_path, SHADOW_REPO_DIR).await?;
                
                // Staging requires relative paths
                index.add_path(&shadow_path.strip_prefix(SHADOW_REPO_DIR)?)?;
            }
        }

        // Handle created files and directories
        for cwd_path in res.missing_left.iter() {
            copy_file_to_dir(os, &cwd, cwd_path, SHADOW_REPO_DIR).await?;
            if cwd_path.is_file() {
                self.modified_map.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, cwd_path).await?);

                // Staging requires relative paths
                index.add_path(&cwd_path.strip_prefix(&cwd)?)?;
            }
        }

        // Handle deleted files
        for shadow_path in res.missing_right.iter() {
            // If path is directory, delete and stage if needed
            if shadow_path.is_dir() {
                os.fs.remove_dir_all(shadow_path).await?;

                // Staging requires relative paths
                index.remove_path(shadow_path.strip_prefix(SHADOW_REPO_DIR)?)?;
                continue;
            }

            // Update table and shadow repo if deleted
            // FIX: removing the entry is probably not the best choice?
            self.modified_map.remove(&shadow_path.to_path_buf());
            os.fs.remove_file(shadow_path).await?;

            // Staging requires relative paths
            index.remove_path(shadow_path.strip_prefix(SHADOW_REPO_DIR)?)?;
        }
        index.write()?;
        Ok(())
    }

    pub async fn create_snapshot(&mut self, os: &Os, message: &str) -> Result<Oid> {
        
        if !self.are_tables_synced() {
            bail!("Tables are not synced! Clean and re-initialize to use snapshots again.");
        }
        
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
        self.snapshot_table.push(Snapshot {
            oid,
            timestamp: signature.when().seconds() as u64,
            message: message.to_string(),
            messages_since: 0,
        });
        self.oid_table.push(oid);
        self.snapshot_count += 1;

        Ok(oid)
    }

    pub async fn restore(&mut self, os: &Os, conversation: &mut ConversationState, number: usize) -> Result<Oid> {
        let oid = match self.oid_table.get(number) {
            Some(s) => s,
            None => bail!("Commit not found in map"),
        };

        // Undo conversation history
        for snapshot in &self.snapshot_table[number..self.snapshot_count] {
            println!("Popping! {} left", conversation.get_history_len());
            for _ in 0..snapshot.messages_since {
                conversation.pop_from_history().ok_or(eyre!("Tried to pop from empty history"))?;
            }
        }

        let oid = self.reset_hard(&oid.to_string()).await?;

        let cwd = os.env.current_dir()?;
        let ignores = RegexSet::new(&[r".git"])?;
        let comparison = Comparison::new(ignores);
        let res = comparison.compare(SHADOW_REPO_DIR, cwd.to_str().unwrap())?;

        // Restore modified files
        for shadow_path in res.changed.iter() {
            if shadow_path.is_file() {
                let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
                copy_file_to_dir(os, SHADOW_REPO_DIR, shadow_path, &cwd).await?;
                self.modified_map.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, &cwd_path).await?);
            }
        }

        // Create missing files and directories
        for shadow_path in res.missing_right.iter() {
            let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
            copy_file_to_dir(os, SHADOW_REPO_DIR, shadow_path, &cwd).await?;
            self.modified_map.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, &cwd_path).await?);

        }

        // Delete extra files
        for cwd_path in res.missing_left.iter() {
            if cwd_path.is_file() {
                os.fs.remove_file(cwd_path).await?;
            } else {
                os.fs.remove_dir_all(cwd_path).await?;
            }
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

    pub async fn clean_all(os: &Os) -> Result<()> {
        os.fs.remove_dir_all(SHADOW_REPO_DIR).await?;
        Ok(())
    }

    pub fn get_latest_snapshot(&mut self) -> Option<&mut Snapshot> {
        self.snapshot_table.iter_mut().last()
    }

    fn are_tables_synced(&self) -> bool {
        self.snapshot_table.len() == self.oid_table.len() && self.snapshot_table.len() == self.snapshot_count
    }
}

impl Debug for SnapshotManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnapshotManager")
            .field("snapshot_count", &self.snapshot_count)
            .field("snapshot_table", &self.snapshot_table)
            .finish()
    }
}

pub async fn get_modified_timestamp(os: &Os, path: &impl AsRef<Path>) -> Result<u64> {
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