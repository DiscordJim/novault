use std::{collections::HashMap, path::{Path, PathBuf}, str::FromStr};
use anyhow::{Result, anyhow};
use crate::sys::{common::NovState, mk::WrappedKey, procedure::actions::VaultState};


pub struct StateFileHandle {
    path: PathBuf,
    state: HashMap<String, String>
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


pub struct StateFile {
    path: PathBuf
}


impl StateFile {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf()
        }
    }
    pub fn set_state(&self, state: NovState) -> Result<()> {
        let mut read = read_hashmap(&self.path)?;

        let text = match state {
            NovState::Uninit => {
                return Err(anyhow!("You cannot set the state to \"uninit\" explicitly."));
            }
            NovState::Init => "init",
            NovState::Sealed => "sealed",
            NovState::Unsealed => "unsealed"
        };

        read.insert("state".to_string(), text.to_string());


        // write_meta_status(&self.path, &read)?;


        Ok(())
    }
    pub fn set_remote(&self, url: &str) -> Result<()> {
        let mut read = read_hashmap(&self.path)?;
        read.insert("remote".to_string(), url.to_string());
        // write_meta_status(&self.path, &read)?;
        Ok(())
    }
    pub fn get_remote(&self) -> Result<Option<String>> {
        let read = read_hashmap(&self.path)?;
        Ok(read.get("remote").cloned())
    }
    pub fn set_mk(&self, salt: &WrappedKey) -> Result<()> {
        let mut read = read_hashmap(&self.path)?;

        read.insert("wrapped".to_string(), salt.to_hex());
        // write_meta_status(&self.path, &read)?;
        Ok(())
    }
    pub fn get_mk(&self) -> Result<WrappedKey> {
        let read = read_hashmap(&self.path)?;

        
        Ok(WrappedKey::from_hex(&read.get("wrapped").ok_or_else(|| anyhow!("Could not lookup salt."))?)?)
    }
    pub fn get_state(&self) -> Result<NovState> {
        let read = read_hashmap(&self.path)?;

        match read.get("state") {
            Some(val) => if val.eq_ignore_ascii_case("init") {
                Ok(NovState::Init)
            } else if val.eq_ignore_ascii_case("sealed") {
                Ok(NovState::Sealed)
            } else if val.eq_ignore_ascii_case("unsealed") {
                Ok(NovState::Unsealed)
            } else {
                Err(anyhow!("Failed to parse the state, read {val}"))
            },
            None => Err(anyhow!("Could not read the state."))
        }
    }
}


fn read_hashmap(root: impl AsRef<Path>) -> Result<HashMap<String, String>> {
    let path = root.as_ref().join(".nov").join(".state");

    if !path.exists() {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        // Write an empty key file.
        std::fs::write(path, &[])?;
        return Ok(HashMap::default());
    }


    let src_str = std::fs::read_to_string(path)?;


    let file = src_str
        .split("\n")
        .map(|f| f.trim())
        .filter(|f| f.len() > 0)
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