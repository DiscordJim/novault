use std::path::Path;

use anyhow::{Result, anyhow};


use crate::{console_log, sys::{common::prompt_password, lib::path::RootPath, procedure::{actions::Context, sequence::{INIT_FULL, Playable}}}};





pub fn run_init(root: impl AsRef<Path>) -> Result<()> {
    let path = root.as_ref();


    if RootPath::new(path).metadata_folder().exists() {
        return Err(anyhow!("There is already a repository in that directory."));
    }



    let mut password = prompt_password(true)?;

    let root = RootPath::new(path);

    INIT_FULL.play(&root, &mut Context::new(&root, &mut password)?)?;

    console_log!(Info, "Succesfully initialized a new NoVault");
    Ok(())
}