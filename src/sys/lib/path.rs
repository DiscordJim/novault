use std::{marker::PhantomData, path::{Path, PathBuf}};
use anyhow::Result;

pub struct RootPath<D>(PathBuf, PhantomData<D>);

pub struct Normal;
pub struct Canonical;

impl<T> RootPath<T> {
    pub fn new(buf: PathBuf) -> RootPath<Normal> {
        RootPath(buf, PhantomData)
    }
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
    pub fn gitattributes(&self) -> PathBuf {
        self.path().join(".gitattributes")
    }
    pub fn inprogress_vault(&self) -> PathBuf {
        self.metadata_folder().join("inpro.zip")
    }
    pub fn vault_binary(&self) -> PathBuf {
        self.path().join("vault.bin")
    }
}