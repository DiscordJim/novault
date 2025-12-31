use std::{collections::HashMap, path::{Path, PathBuf}};

use anyhow::Result;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::Deserialize;


fn get_gitignore(root: impl AsRef<Path>) -> Result<Gitignore> {
    let path = root.as_ref();

     let gi_path = path.join(".gitignore");
    

     // First we read the gitignore (if it exists).
    let mut builder = GitignoreBuilder::new(&root);
    if gi_path.exists() {
        builder.add(gi_path.clone());
    }

    let gignore = builder.build()?;
    Ok(gignore)
}


#[derive(Clone, Copy, Debug, Deserialize, Default)]
pub enum FilterDecision {
    Encrypt, 
    #[default]
    IgnoreAndEncrypt,
    Delete,
    Unsecure
}

pub struct NovFilter {
    root: PathBuf,
    git_ignore: Gitignore,
    rules: TomlRules
}

struct TomlRules {
    default_policy: FilterDecision,
    unsecured: Option<Gitignore>,
    delete: Option<Gitignore>
}

#[derive(Deserialize, Debug)]
struct TomlRulesSer {
    settings: SettingsSer,
    rules: HashMap<String, Vec<String>>
}

#[derive(Deserialize, Debug)]
struct SettingsSer {
    #[serde(default)]
    default_policy: FilterDecision
}

fn read_rules(path: impl AsRef<Path>) -> Result<TomlRules> {

    let toml = path.as_ref().join("novault.toml");

    if !toml.exists() {
        // TODO
        return Ok(TomlRules {
            default_policy: FilterDecision::default(),
            delete: None,
            unsecured: None
        })
    }

    let string = std::fs::read_to_string(&toml)?;

    let cfg: TomlRulesSer = toml::from_str(&string)?;
    // println!("Config: {:?}", cfg);

    let mut delete = None;
    let mut unsecured = None;

    if let Some(unsec) = cfg.rules.get("unsecured") {
        let mut gibuilder = GitignoreBuilder::new("");
        for unsecured in unsec {
            gibuilder.add_line(None, unsecured)?;
        }
        unsecured = Some(gibuilder.build()?);
    }

    if let Some(unsec) = cfg.rules.get("delete") {
        let mut gibuilder = GitignoreBuilder::new("");
        for unsecured in unsec {
            gibuilder.add_line(None, unsecured)?;
        }
        delete = Some(gibuilder.build()?);
    }

    // std::process::exit(1);
    Ok(TomlRules {
        default_policy: cfg.settings.default_policy,
        delete,
        unsecured
    })
}

impl NovFilter {
    pub fn from_root(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().canonicalize()?;
        let git = get_gitignore(&path)?;

        let toml = read_rules(&path)?;

        Ok(Self {
            root: path.to_path_buf(),
            git_ignore: git,
            rules: toml
        })
    }
    pub fn check_decision(&self, path: impl AsRef<Path>) -> Result<FilterDecision> {
        let rel = path.as_ref().strip_prefix(&self.root)?;

        if let Some(delete) = &self.rules.delete && delete.matched(rel, rel.is_dir()).is_ignore() {
                return Ok(FilterDecision::Delete);
            
        }

        if let Some(delete) = &self.rules.unsecured && delete.matched(rel, rel.is_dir()).is_ignore() {
                return Ok(FilterDecision::Unsecure);
            }
        


        if self.git_ignore.matched(rel, rel.is_dir()).is_ignore() {
            Ok(self.rules.default_policy)
        } else {
            Ok(FilterDecision::Encrypt)
        }
    }
    
}