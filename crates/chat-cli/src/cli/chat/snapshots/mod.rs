use std::collections::HashMap;
use std::convert;
use std::path::{
    Path,
    PathBuf,
};
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use dircmp::Comparison;
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
use regex::RegexSet;
use walkdir::WalkDir;

use crate::cli::ConversationState;
use crate::os::Os;

// ######## HARDCODED VALUES ########
const SHADOW_REPO_DIR: &str = "/Users/kiranbug/.aws/amazonq/shadow";
// ######## ---------------- ########

pub struct SnapshotManager {
    pub repo: Repository,
    pub snapshot_map: HashMap<Oid, Snapshot>,
    pub latest_snapshot: Oid,
    pub snapshot_count: u64,

    // Contains modification timestamps for absolute paths in cwd
    pub modified_table: HashMap<PathBuf, u64>,

}

pub struct Snapshot {
    pub oid: Oid,
    pub timestamp: u64,
    pub message: String,
    pub index: u64,
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

        // // Copy everything into the shadow repo
        // for entry in WalkDir::new(&cwd)
        //     .into_iter()
        //     .filter_entry(|e| !path_contains(e.path(), ".git"))
        //     .skip(1)
        // {
        //     let entry = match entry {
        //         Ok(entry) => entry,
        //         Err(_) => {
        //             // FIX: what do we do here instead of silently failing?
        //             continue;
        //         },
        //     };
        //     let path = entry.path();
        //     copy_file_to_dir(os, &cwd, path, SHADOW_REPO_DIR).await?;
        // }

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
    /// so users don't have to wait if nothing was modified
    pub async fn any_modified(&self, os: &Os) -> Result<bool> {
        let ignores = RegexSet::new(&[r".git"]).expect("should compile");
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

        println!("{res:#?}");

        // Handle modified files
        for shadow_path in res.changed.iter() {
            println!("Staging modified: {}", shadow_path.display());
            if shadow_path.is_file() {
                let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
                self.modified_table.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, &cwd_path).await?);
                copy_file_to_dir(os, &cwd, cwd_path, SHADOW_REPO_DIR).await?;
                
                // Staging requires relative paths
                index.add_path(&shadow_path.strip_prefix(SHADOW_REPO_DIR)?)?;
            }
        }

        // Handle created files and directories
        for cwd_path in res.missing_left.iter() {
            println!("Created file: {}", &cwd_path.display());
            copy_file_to_dir(os, &cwd, cwd_path, SHADOW_REPO_DIR).await?;
            if cwd_path.is_file() {
                self.modified_table.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, cwd_path).await?);

                // Staging requires relative paths
                index.add_path(&cwd_path.strip_prefix(&cwd)?)?;
            }
        }

        // Handle deleted files
        for shadow_path in res.missing_right.iter() {
            println!("Staging deleted: {}", shadow_path.display());
            // If path is directory, delete and stage if needed
            if shadow_path.is_dir() {
                os.fs.remove_dir_all(shadow_path).await?;

                // Staging requires relative paths
                index.remove_path(shadow_path.strip_prefix(SHADOW_REPO_DIR)?)?;
                continue;
            }

            // Update table and shadow repo if deleted
            // FIX: removing the entry is probably not the best choice?
            self.modified_table.remove(&shadow_path.to_path_buf());
            os.fs.remove_file(shadow_path).await?;

            // Staging requires relative paths
            index.remove_path(shadow_path.strip_prefix(SHADOW_REPO_DIR)?)?;
        }
        index.write()?;
        Ok(())
    }

    pub async fn create_snapshot(&mut self, os: &Os, message: &str) -> Result<Oid> {
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
            index: self.snapshot_count,
        });
        self.latest_snapshot = oid;
        self.snapshot_count += 1;

        Ok(oid)
    }

    pub async fn restore(&mut self, os: &Os, conversation: &mut ConversationState, commit_hash: &str) -> Result<Oid> {
        let snapshot = match self.snapshot_map.get(&Oid::from_str(commit_hash)?) {
            Some(s) => s,
            None => bail!("Commit not found in map"),
        };
        let restore_index = snapshot.index;
        let cwd = os.env.current_dir()?;

        let oid = self.reset_hard(commit_hash).await?;

        let ignores = RegexSet::new(&[r".git"])?;
        let comparison = Comparison::new(ignores);
        let res = comparison.compare(SHADOW_REPO_DIR, cwd.to_str().unwrap())?;

        println!("DIFF: {:#?}", res);

        // Restore modified files
        for shadow_path in res.changed.iter() {
            if shadow_path.is_file() {
                let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
                copy_file_to_dir(os, SHADOW_REPO_DIR, shadow_path, &cwd).await?;
                self.modified_table.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, &cwd_path).await?);
            }
        }

        // Create missing files and directories
        for shadow_path in res.missing_right.iter() {
            let cwd_path = convert_path(SHADOW_REPO_DIR, shadow_path, &cwd)?;
            copy_file_to_dir(os, SHADOW_REPO_DIR, shadow_path, &cwd).await?;
            self.modified_table.insert(cwd_path.to_path_buf(), get_modified_timestamp(os, &cwd_path).await?);

        }

        // Delete extra files
        for cwd_path in res.missing_left.iter() {
            if cwd_path.is_file() {
                os.fs.remove_file(cwd_path).await?;
            } else {
                os.fs.remove_dir_all(cwd_path).await?;
            }
        }

        // FIX: terrible workaround for popping context
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
