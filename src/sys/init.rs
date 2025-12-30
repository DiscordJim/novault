use std::{ffi::OsStr, path::Path};

use anyhow::{Result, anyhow};


use crate::{console_log, sys::{common::{NovState, exists_git_repo, exists_metadata_directory, make_git_repo, prompt_password}, lib::path::RootPath, mk::{MasterVaultKey, UserVaultKey, WrappedKey}, procedure::{actions::{Context, VaultState}, sequence::{INIT_FULL, Playable}}, statefile::StateFile}};





pub fn run_init(root: impl AsRef<Path>) -> Result<()> {
    let path = root.as_ref();


    // if exists_metadata_directory(path) {
    //     return Err(anyhow!("The target is already initialized."));
    // } else {
    //     // if !exists_git_repo(path) {
    //     //     console_log!(Info, "Initializing a new git repository.");
    //     //     make_git_repo(path)?;
    //     // } else {
    //     //     console_log!(Info, "There is already an existing git repository.");
    //     // }

    //     // Now we create the actual metadata directory.
    //     create_metadata_directory(path)?;
        
    // }

    let mut password = prompt_password(true)?;

    let root = RootPath::new(path.to_path_buf());

    INIT_FULL.play(&root, &mut Context::new(&root, &mut password)?)?;

    console_log!(Info, "Succesfully initialized a new NoVault");
    Ok(())
}