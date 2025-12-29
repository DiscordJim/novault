use std::{ffi::OsStr, path::Path};

use anyhow::{Result, anyhow};


use crate::{console_log, sys::{common::{NovState, exists_git_repo, exists_metadata_directory, make_git_repo, prompt_password, seal_postmigrate}, mk::{MasterVaultKey, UserVaultKey, WrappedKey}, statefile::StateFile}};

#[cfg(windows)]
fn to_wide(s: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}



fn create_metadata_directory(root: impl AsRef<Path>) -> Result<()> {


  
    let state_file = StateFile::new(root.as_ref());

    if exists_metadata_directory(root.as_ref()) {
        return Err(anyhow!("Tried to create a metadata directory but one already exists."));
    }

    let mut password = prompt_password(true)?;


    let master  = MasterVaultKey::generate();

    let wrapped = WrappedKey::init(&UserVaultKey::init_fresh(&mut password)?, &master)?;

    // Create an atomic file.
    state_file.set_state(NovState::Init)?;



    let met_path = root.as_ref().join(".nov");
    #[cfg(windows)]
    {
        // On Windows you need special logic to make a directory properly hidden.
        use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, SetFileAttributesW};

        
        // Mark the file as hidden.
        if unsafe { SetFileAttributesW(to_wide(met_path.as_os_str()).as_ptr(), FILE_ATTRIBUTE_HIDDEN) } == 0 {
            use windows_sys::Win32::Foundation::GetLastError;

            let err = unsafe { GetLastError() };
            return Err(std::io::Error::from_raw_os_error(err as i32))?;
        }
    }




    let ig_path = root.as_ref().join(".gitignore");
    if !ig_path.exists() {
        std::fs::write(&ig_path, "# Feel free to customize.\n\n# Leave the next line be.\n/.nov\n")?;
    }

    // Perform the initial seal, we will use this to seed the git
    // repository.
    seal_postmigrate(root.as_ref(), &master)?;
  
    // Now we make the external git repository.
    make_git_repo(root.as_ref())?;


    // Write that we are initializing.
    state_file.set_state(NovState::Sealed)?;


    state_file.set_mk(&wrapped)?;


    Ok(())
}

pub fn run_init(root: impl AsRef<Path>) -> Result<()> {
    let path = root.as_ref();


    if exists_metadata_directory(path) {
        return Err(anyhow!("The target is already initialized."));
    } else {
        if !exists_git_repo(path) {
            console_log!(Info, "Initializing a new git repository.");
            make_git_repo(path)?;
        } else {
            console_log!(Info, "There is already an existing git repository.");
        }

        // Now we create the actual metadata directory.
        create_metadata_directory(path)?;
        
    }

    console_log!(Info, "Succesfully initialized a new NoVault");
    Ok(())
}