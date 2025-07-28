use std::collections::HashMap;
use std::path::{
    Path,
    PathBuf,
};

use eyre::{
    Result,
    bail,
};
use serde::{
    Deserialize,
    Serialize,
};
use sha2::{
    Digest,
    Sha256,
};
use tracing::debug;

use crate::cli::chat::tools::sanitize_path_tool_arg;
use crate::os::Os;

// ######## HARDCODED VALUES ########
const CHECKPOINT_DIR: &str = "/Users/kiranbug/.aws/amazonq/checkpoints/first/";
const CHECKPOINT_FILE: &str = "/Users/kiranbug/.aws/amazonq/checkpoints/test_file.json";
// ######## ---------------- ########

// FIX: Move complicated logic (None -> Some) into checkpoint function?
//      - The benefit is that calling new_checkpoint() will be cleaner
//      - The downside is that the function will be messier and may miss edge cases; maybe better to handle on a case-by-case basis?
// FIX: Remove hardcoded values
// FIX: 

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CheckpointManager {
    checkpoints: Vec<Checkpoint>,
    turn_indices: Vec<usize>,
    store_dir: PathBuf,

    first_occurrence_cache: HashMap<PathBuf, usize>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub state: HashMap<PathBuf, Option<ContentHash>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ContentHash(String);

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CheckpointManager {
    pub async fn init(os: &Os) -> Result<()> {
        let manager = Self {
            store_dir: Self::get_new_directory(),
            ..Default::default()
        };
        Self::save_manager(os, &manager).await?;
        Ok(())
    }

    pub async fn load_manager(os: &Os) -> Result<Self> {
        Ok(serde_json::from_str::<CheckpointManager>(
            &os.fs.read_to_string(CHECKPOINT_FILE).await?,
        )?)
    }

    pub async fn save_manager(os: &Os, manager: &CheckpointManager) -> Result<()> {
        os.fs.write(CHECKPOINT_FILE, serde_json::to_string(manager)?).await?;
        Ok(())
    }

    pub async fn new_checkpoint(&mut self, os: &Os, path: PathBuf, data: Option<Vec<u8>>) -> Result<()> {
        let hash = data.as_ref().map(hash_contents);

        // Create obj file
        if let Some((hash, data)) = hash.as_ref().zip(data.as_ref()) {
            self.create_obj_if_needed(os, &hash, data).await?;
        }

        // Copy and update the previous checkpoint's state
        let mut new_map = if let Some(checkpoint) = self.checkpoints.iter().last() {
            checkpoint.state.clone()
        } else {
            HashMap::new()
        };
        new_map.insert(path.clone(), hash);

        self.checkpoints.push(Checkpoint { state: new_map });

        // Log first occurrence
        self.first_occurrence_cache
            .entry(path)
            .or_insert(self.checkpoints.len() - 1);

        Ok(())
    }

    pub async fn restore(&mut self, os: &Os, index: usize) -> Result<()> {
        let checkpoint = match self.checkpoints.get(index) {
            Some(c) => c,
            None => bail!(format!("No checkpoint with index: {index}")),
        };
        // If a touched file isn't in this checkpoint's state, look forward in history
        // to see the first time the file shows up.
        // This is complicated but necessary given the current design.
        for path in self.first_occurrence_cache.keys() {
            let hash_option = match checkpoint.state.get(path) {
                Some(hash) => hash,
                None => {
                    let first_occurrence = self.first_occurrence_cache.get(path).unwrap();
                    self.checkpoints[*first_occurrence].state.get(path).unwrap()
                },
            };
            match hash_option {
                Some(hash) => self.restore_file(os, path, hash).await?,
                None if os.fs.exists(path) => os.fs.remove_file(path).await?,
                _ => (),
            };
        }
        Ok(())
    }

    async fn create_obj_if_needed(&self, os: &Os, hash: &ContentHash, data: impl AsRef<[u8]>) -> Result<()> {
        let obj_path = self.hash_to_obj_path(hash);
        if os.fs.exists(&obj_path) {
            return Ok(());
        }
        os.fs.create_new(&obj_path).await?;
        os.fs.write(obj_path, data).await?;
        Ok(())
    }

    fn hash_to_obj_path(&self, hash: &ContentHash) -> PathBuf {
        self.store_dir.join(hash.to_string())
    }

    fn get_new_directory() -> PathBuf {
        PathBuf::from(CHECKPOINT_DIR)
    }

    async fn restore_file(&self, os: &Os, path: impl AsRef<Path>, hash: &ContentHash) -> Result<()> {
        let path = path.as_ref();
        if !os.fs.exists(&path) {
            os.fs.create_new(&path).await?;
        }
        os.fs.copy(self.hash_to_obj_path(hash), path).await?;
        Ok(())
    }

    pub fn is_tracked(&self, path: impl AsRef<Path>) -> bool {
        self.first_occurrence_cache.contains_key(&path.as_ref().to_path_buf())
    }
}

fn hash_contents(data: impl AsRef<[u8]>) -> ContentHash {
    let hash = Sha256::digest(data);
    ContentHash(hash.iter().map(|b| format!("{:02x}", b)).collect())
}

pub async fn setup_checkpoint_tracking(os: &Os, path: impl AsRef<Path>) -> Result<Option<(CheckpointManager, PathBuf)>> {
    let manager = match CheckpointManager::load_manager(os).await {
        Ok(m) => m,
        Err(_) => {
            debug!("No checkpoint manager initialized; tool call will not be tracked");
            return Ok(None);
        },
    };

    let checkpoint_path = sanitize_path_tool_arg(os, path);
    let canonical_path = match checkpoint_path.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            debug!("Path could not be canonicalized; tool call will not be tracked");
            return Ok(None);
        },
    };

    Ok(Some((manager, canonical_path)))
}
