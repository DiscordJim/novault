use std::{marker::PhantomData, path::{Path, PathBuf}};
use anyhow::Result;


pub struct RootPath<D>(PathBuf, PhantomData<D>);

impl<D> Clone for RootPath<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

pub struct Normal;
pub struct Canonical;

impl RootPath<Normal> {
    pub fn new(buf: impl AsRef<Path>) -> RootPath<Normal> {
        RootPath(buf.as_ref().to_path_buf(), PhantomData)
    }
}

impl<T> RootPath<T> {
    
    pub fn canonicalize(&self) -> Result<RootPath<Canonical>> {
        Ok(RootPath(self.0.clone().canonicalize()?, PhantomData))
    }
    pub fn path(&self) -> &PathBuf {
        &self.0
    }
    pub fn metadata_folder(&self) -> PathBuf {
        self.path().join(".nov")
    }
    pub fn unsecure_folder(&self) -> PathBuf {
        self.metadata_folder().join("unsecure")
    }
    pub fn secure_local_folder(&self) -> PathBuf {
        self.metadata_folder().join("secure_local")
    }
    pub fn secure_local_zip(&self) -> PathBuf {
        self.secure_local_folder().join("inpro.bin")
    }
    pub fn deletion_shards(&self) -> PathBuf {
        self.metadata_folder().join(".delete")
    }
    pub fn gitignore(&self) -> PathBuf {
        self.path().join(".gitignore")
    }
    pub fn config(&self) -> PathBuf {
        self.path().join("novault.toml")
    }
    pub fn gitattributes(&self) -> PathBuf {
        self.path().join(".gitattributes")
    }
    pub fn inprogress_vault(&self) -> PathBuf {
        self.metadata_folder().join("inpro.zip")
    }
    pub fn vault_binary(&self) -> PathBuf {
        self.path().join("vault.bin")
    }
    pub fn wrap_folder(&self) -> PathBuf {
        self.metadata_folder().join("wrap")
    }
    pub fn external_git(&self) -> PathBuf {
        self.wrap_folder().join("external.git")
    }
    pub fn local_git(&self) -> PathBuf {
        self.path().join(".git")
    }
}