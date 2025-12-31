use std::{collections::HashMap, path::{Path, PathBuf}, str::FromStr};
use anyhow::{Result, anyhow};
use strum::EnumString;
use crate::sys::{mk::WrappedKey, procedure::actions::VaultState};


pub struct StateFileHandle {
    path: PathBuf,
    state: HashMap<String, String>
}

#[derive(EnumString, strum::AsRefStr, PartialEq, Eq)]
pub enum SyncMethod {
    Git,
    TigrisS3

}

impl StateFileHandle {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let mut obj = Self {
            path: path.as_ref().to_path_buf(),
            state: HashMap::default()
        };
        obj._load()?;
        Ok(obj)
    }
    fn _load(&mut self)  -> Result<()> {
        self.state = read_hashmap(&self.path)?;
        Ok(())
    }
    pub fn reload(&mut self) -> Result<()> {
        self._load()
    }
    pub fn set_state(&mut self, state: VaultState) {
        self.state.insert("state".to_string(), format!("{state:?}"));
    }
    pub fn set_init(&mut self, init: bool) {
        self.state.insert("init".to_string(), format!("{init:?}"));

    }
    pub fn get_init(&self) -> Result<bool> {
        Ok(self.state.get("init").map(|f| bool::from_str(f)).ok_or_else(|| anyhow!("Could not find 'init' in state file."))??)
    }
    pub fn set_remote(&mut self, url: &str) {
        self.state.insert("remote".to_string(), url.to_string());
    }
    pub fn set_remote_storage(&mut self, method: SyncMethod) {
        self.state.insert("remote_backend".to_string(), method.as_ref().to_string());
    }
    pub fn previous_tigris_commit_stamp(&mut self, stamp: &str) {
        // println!("Stamp: {stamp}");
        self.state.insert("prev_stamp_t3".to_string(), stamp.to_string());
    }
    pub fn get_previous_tigris_commit_stamp(&self) -> Result<String> {
        self.state.get("prev_stamp_t3").ok_or_else(|| anyhow!("Failed to get the Tigris commit stamp.")).cloned()
    }
    pub fn get_remote_storage(&self) -> Result<Option<SyncMethod>> {
        match self.state.get("remote_backend") {
            Some(v) => Ok(Some(SyncMethod::from_str(v)?)),
            None => Ok(None)
        }
    }
    pub fn get_remote(&self) -> Option<String> {
        self.state.get("remote").map(|f| f.to_string())
    }
    pub fn set_master_key(&mut self, key: &WrappedKey) {
        self.state.insert("wrapped".to_string(), key.to_hex());
    }
    pub fn get_wrapped_key(&self) -> Result<WrappedKey> {
        WrappedKey::from_hex(self.state.get("wrapped").ok_or_else(|| anyhow!("Failed to lookup the wrapped key."))?)
    }
    pub fn get_state(&mut self) -> Result<VaultState> {
        match self.state.get("state") {
            Some(v) => Ok(VaultState::from_str(v)?),
            None => Ok(VaultState::Uninit)
        }
        // Ok(VaultState::from_str(self.state.get("state").ok_or_else(|| anyhow!("Failed to get the state."))?)?)
    }
    pub fn writeback(&mut self) -> Result<()> {
        write_meta_status(&self.path, &self.state)?;

        Ok(())
    }
}




pub fn read_hashmap(root: impl AsRef<Path>) -> Result<HashMap<String, String>> {
    let path = root.as_ref().join(".nov").join(".state");

    if !path.exists() {
        if let Some(parent) = path.parent()  && !parent.exists() {
           
                std::fs::create_dir_all(parent)?;
            
        }
        // Write an empty key file.
        std::fs::write(path, [])?;
        return Ok(HashMap::default());
    }


    let src_str = std::fs::read_to_string(path)?;


    let file = src_str
        .split("\n")
        .map(|f| f.trim())
        .filter(|f| !f.is_empty())
        .map(|f| f.split("="))
        .flat_map(|mut f| Some((f.next()?.to_string(), f.next()?.to_string())))
        .collect();


    Ok(file)
}


fn write_meta_status(root: impl AsRef<Path>, bytes: &HashMap<String, String>) -> Result<()> {


    // println!("WRITING....");
    let root = root.as_ref().join(".nov");

    let data = bytes.iter().map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n");

    if !root.exists() {
        std::fs::create_dir_all(&root)?;
    }

    let temp = root.join(".state.temp");
    // println!("TEMP: {:?}", temp);
    std::fs::write(&temp, data.as_bytes())?;

    // println!("WROTE 2");

    atomicwrites::replace_atomic(temp.as_ref(), &root.join(".state"))?;

    Ok(())
}