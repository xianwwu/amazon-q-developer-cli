use std::collections::{
    HashMap,
    HashSet,
};
use std::path::{
    Path,
    PathBuf,
};

use eyre::{
    Result,
    bail,
};
use sha2::{
    Digest,
    Sha256,
};
use serde::{
    Serialize,
    Deserialize
};

use crate::os::Os;

// ######## HARDCODED VALUES ########
const CHECKPOINT_DIR: &str = "/Users/kiranbug/.aws/amazonq/checkpoints/first/";
const CHECKPOINT_FILE: &str = "/Users/kiranbug/.aws/amazonq/checkpoints/test_file.json";
// ######## ---------------- ########

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CheckpointManager {
    checkpoints: Vec<Checkpoint>,
    turn_indices: Vec<usize>,
    store_dir: PathBuf,

    // This should always be a superset of the  
    // keys of any checkpoint's latest_state
    touched_files: HashSet<PathBuf>,

    index_map: HashMap<String, usize>,
    
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    path: PathBuf,
    hash: String,
    latest_state: HashMap<PathBuf, usize>,
    kind: CheckpointType,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CheckpointType {
    Origin,
    Modify,
    Delete,
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
        Ok(serde_json::from_str::<CheckpointManager>(&os.fs.read_to_string(CHECKPOINT_FILE).await?)?)
    }

    pub async fn save_manager(os: &Os, manager: &CheckpointManager) -> Result<()> {
        os.fs.write(CHECKPOINT_FILE, serde_json::to_string(manager)?).await?;
        Ok(())
    }

    pub async fn on_origin(&mut self, os: &Os, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let data = os.fs.read(&path).await?;
        let hash = hash_contents(&data);

        self.create_obj_if_needed(os, &hash, data).await?;
        self.checkpoints.push(Checkpoint {
            path: path.to_path_buf(),
            hash,
            latest_state: self.get_new_latest_state(path.to_path_buf()),
            kind: CheckpointType::Origin,
        });
        self.touched_files.insert(path.to_path_buf());

        Ok(())
    }

    pub async fn on_modification(&mut self, os: &Os, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        // Need original contents to restore
        if !self.touched_files.contains(path) {
            bail!("Original contents are not tracked!");
        }
        let data = os.fs.read(&path).await?;
        let hash = hash_contents(&data);

        self.create_obj_if_needed(os, &hash, data).await?;
        self.checkpoints.push(Checkpoint {
            path: path.to_path_buf(),
            hash,
            latest_state: self.get_new_latest_state(path.to_path_buf()),
            kind: CheckpointType::Modify,
        });

        Ok(())
    }

    pub async fn on_deletion(&mut self, os: &Os, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        // Need original contents to restore
        if !self.touched_files.contains(path) {
            bail!("Original contents are not tracked!");
        }
        self.checkpoints.push(Checkpoint {
            path: path.to_path_buf(),
            hash: String::new(),
            latest_state: self.get_new_latest_state(path.to_path_buf()),
            kind: CheckpointType::Delete,
        });
        Ok(())
    }

    pub async fn restore(&mut self, os: &Os, index: usize) -> Result<()> {
        let checkpoint = match self.checkpoints.get(index) {
            Some(c) => c,
            None => bail!("Invalid checkpoint index")
        };

        for path in self.touched_files.clone() {
            
            // If the file hadn't been created by that point
            if !checkpoint.latest_state.contains_key(&path) {
                if os.fs.exists(&path) {
                    os.fs.remove_file(&path).await?;
                }
                self.touched_files.remove(&path);
                continue;
            }

            let index = checkpoint.latest_state.get(&path).unwrap();
            let latest = &self.checkpoints[*index];
            match latest.kind {
                CheckpointType::Origin | CheckpointType::Modify => {
                    if !os.fs.exists(&path) {
                        os.fs.create_new(&path).await?;
                    }
                    os.fs.copy(self.hash_to_obj_path(&checkpoint.hash), path).await?;
                },
                CheckpointType::Delete => {
                    if os.fs.exists(&path) {
                        os.fs.remove_file(&path).await?;
                    }
                }
            }
        }

        self.checkpoints.truncate(index + 1);
        Ok(())
    }

    pub fn is_tracking(&self, path: impl AsRef<Path>) -> bool {
        self.touched_files.contains(path.as_ref())
    }

    async fn create_obj_if_needed(&self, os: &Os, hash: &String, data: Vec<u8>) -> Result<()> {
        let obj_path = self.hash_to_obj_path(hash);
        if os.fs.exists(&obj_path) {
            return Ok(());
        }
        os.fs.create_new(&obj_path).await?;
        os.fs.write(obj_path, data).await?;
        Ok(())
    }

    /// Returns a new updated state. Call this BEFORE adding a checkpoint to the list
    fn get_new_latest_state(&self, path: PathBuf) -> HashMap<PathBuf, usize> {
        let mut map = if self.checkpoints.is_empty() {
            HashMap::new()
        } else {
            self.checkpoints.iter().last().unwrap().latest_state.clone()
        };
        map.insert(path, self.checkpoints.len());
        map
    }

    fn hash_to_obj_path(&self, hash: &String) -> PathBuf {
        self.store_dir.join(hash)
    }

    fn get_new_directory() -> PathBuf {
        PathBuf::from(CHECKPOINT_DIR)
    }
}

fn hash_contents(data: &Vec<u8>) -> String {
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}
