use anyhow::{Result, anyhow};
use walkdir::WalkDir;
use zip::ZipArchive;
#[cfg(windows)]
use std::ffi::OsStr;
use std::{fs::File, io::{BufRead, BufReader, BufWriter, Cursor, Write}, path::{Path, PathBuf}, str::FromStr};

use crate::{console_log, sys::{common::{exists_git_repo, make_git_repo}, filter::{FilterDecision, NovFilter}, lib::path::{Normal, RootPath}, mk::{CachedPassword, MasterVaultKey, UserVaultKey, WrappedKey}, procedure::sequence::{Playable, SEAL_FULL, UNSEAL_FULL}, statefile::{StateFileHandle, SyncMethod}, writer::{VaultWriter, decrypt}}};

#[derive(Debug, Clone, Copy, strum::EnumString, PartialEq, Eq)]
pub enum VaultState {

    Uninit,

    /// This is what the initialization phase
    /// calls in order to supply a wrapped key.
    Seed,
    /// Init filesystem
    InitFileSystem,
    MakeExternalGitRepo,
    MarkInitDone,


    // SEALING
    RecreatingDirectories,
    Encrypting,
    UnlinkPostSeal,
    RelocateEncryptedBinaries,
    WriteMandatoryPostSealFiles,
    RestoreVaultGit,
    Sealed,
   
    // UNSEALING
    DecryptMainVault,
    DecryptLocallySecuredVault,
    StashExternalGitRepo,
    DeleteSealedGitFiles,
    ExpandMainVault,
    ExpandLocalVault,
    CleanupOldBinaries,
    RestoreUnsecureFiles,
    Unsealed

    // SET REMOTE
}

impl VaultState {
    /// These states are ones with no inertia, in which the
    /// system is a stable state.
    /// 
    /// If we ever start in a different state, then we need to
    /// do repairs.
    pub fn is_rest_state(&self) -> bool {
        matches!(self, Self::Uninit | Self::Sealed | Self::Unsealed)
    }
}

pub struct Context<'a> {
    password: &'a mut CachedPassword,
    handle: StateFileHandle,
    master: Option<MasterVaultKey>,
    new_wrapped: Option<WrappedKey>,
    decrypted_zip_bytes: Option<Vec<u8>>,
    decrypted_local_bytes: Option<Vec<u8>>,
    starting: bool,
    skip_local_zip: bool,
    fallthrough: bool
}

impl<'a> Context<'a> {
    pub fn password(&mut self) -> &mut CachedPassword {
        self.password
    }
    pub fn state_file(&self) -> &StateFileHandle {
        &self.handle
    }
    pub fn state_file_mut(&mut self) -> &mut StateFileHandle {
        &mut self.handle
    }
    pub fn new(root: &RootPath<Normal>, pass: &'a mut CachedPassword) -> Result<Self> {
        Ok(Self {
            starting: true,
            password: pass,
            handle: StateFileHandle::new(root.path())?,
            master: None,
            new_wrapped: None,
            decrypted_zip_bytes: None,
            decrypted_local_bytes: None,
            fallthrough: false,
            skip_local_zip: false
        })
    }
}

impl VaultState {
    pub fn act(&self, root: &RootPath<Normal>, master: &mut Context) -> Result<()> {
        self._act(root, master)
            .map_err(|e| anyhow!("({self:?}) {e:?}"))
    }
    fn repair(&self, source: VaultState, root: &RootPath<Normal>, master: &mut Context) -> Result<()> {

        match source {
            Self::Uninit => { /* No repairs needed. */ },
       
            Self::Seed | Self::InitFileSystem => {
                // TOOD: We should probably ask permission to do this?

                if root.metadata_folder().exists() {
                    std::fs::remove_dir_all(root.metadata_folder())?;
                }
                master.handle.set_state(Self::Uninit);
            },

            Self::MakeExternalGitRepo => {
                make_external_git_repo(root)?;
                if master.handle.get_init()? {
                    mark_init_done(master)?;
                }
                master.handle.set_state(Self::Sealed);
            }

            Self::MarkInitDone => {
                mark_init_done(master)?;
                master.handle.set_state(Self::Sealed);
            }


            Self::RecreatingDirectories => {
                recreate_directories(root)?;
                master.handle.set_state(Self::Unsealed);
            }
            

            Self::Encrypting => revert_encryption_state(root, master)?,
            
            Self::DeleteSealedGitFiles => {
                revert_encryption_state(root, master)?;
                create_mandatory_post_seal_files(root)?;
                master.handle.set_state(Self::Unsealed);
            }
            
            Self::UnlinkPostSeal => {
                master.starting = false;
                SEAL_FULL.resume(Self::UnlinkPostSeal)
                    .play(root, master)?;
               
            }
            Self::RelocateEncryptedBinaries => {
                master.starting = false;
                SEAL_FULL.resume(Self::RelocateEncryptedBinaries)
                    .play(root, master)?;
            }
            Self::WriteMandatoryPostSealFiles => {
                master.starting = false;
                SEAL_FULL.resume(Self::WriteMandatoryPostSealFiles)
                    .play(root, master)?;
            }
            Self::RestoreVaultGit => {
                master.starting = false;
                SEAL_FULL.resume(Self::RestoreVaultGit)
                    .play(root, master)?;
            }
            Self::Sealed => { /* Nothing to do */ }

        


            // UNSEALING
            
            Self::DecryptMainVault | Self::DecryptLocallySecuredVault => { 
                master.handle.set_state(Self::Sealed);
            },
            Self::StashExternalGitRepo => {
                restore_vault_git(root, master)?;
                master.handle.set_state(Self::Sealed);
            }
            Self::ExpandMainVault => {
                // TODO: Cleanup files that were produced here.
                master.handle.set_state(Self::Sealed);
            }
            Self::ExpandLocalVault => {
                master.handle.set_state(Self::Sealed);
            }
            Self::CleanupOldBinaries => {
                master.starting = false;
                UNSEAL_FULL.resume(Self::CleanupOldBinaries)
                    .play(root, master)?;
            }
            Self::RestoreUnsecureFiles => {
                master.starting = false;
                UNSEAL_FULL.resume(Self::RestoreUnsecureFiles)
                    .play(root, master)?;
            }
            Self::Unsealed => { /* Nothing to do */}
        }
        Ok(())
    }
    fn _act(&self, root: &RootPath<Normal>, master: &mut Context) ->  Result<()> {
        if master.fallthrough {
            return Ok(());
        }
        if master.starting {
            let mut state = master.handle.get_state()?;
            if !state.is_rest_state() {
                console_log!(Warn, "We are not in a rest state, we were left in an incomplete state ({state:?}).");


                // Perform reparations.
                self.repair(state, root, master)?;
                master.handle.writeback()?;
                state = master.handle.get_state()?;
            }
            if state == VaultState::Unsealed && *self == Self::DecryptMainVault {
                console_log!(Info, "The vault is already unsealed.");
                master.fallthrough = true;
                master.handle.writeback()?;
                return Ok(());
            }
            if state == Self::Sealed && *self == Self::RecreatingDirectories {
                console_log!(Info, "The vault is already sealed.");
                master.fallthrough = true;
                return Ok(());
            }
            master.starting = false;
        }
        

        master.handle.set_state(*self);
        master.handle.writeback()?;
        match self {
            VaultState::Uninit => {}
            VaultState::Seed => {
                seed_state(master)?;
            }
            VaultState::InitFileSystem => {
                init_filesystem(root)?;
            }
            VaultState::MarkInitDone => {
                mark_init_done(master)?;
            }
            VaultState::MakeExternalGitRepo => {
                make_external_git_repo(root)?;
            }
            VaultState::RecreatingDirectories => {
                recreate_directories(root)?;
            }
            VaultState::Encrypting => {
                // return Err(anyhow!("breh"));
                write_encrypted_archives(root, master)?;
            }

            VaultState::UnlinkPostSeal => {
                unlink_other_archives(root)?;
            }
            VaultState::RelocateEncryptedBinaries => {
                relocate_encrypted_binaries(root)?;
            }
            VaultState::WriteMandatoryPostSealFiles => {
                create_mandatory_post_seal_files(root)?;
            }
            VaultState::RestoreVaultGit => {
                restore_vault_git(root, master)?;
            },
            VaultState::Sealed => {},
            
            VaultState::DecryptMainVault => {
                decrypt_main_vault(root, master)?;
            }
            VaultState::DecryptLocallySecuredVault => {
                decrypt_local_vault(root, master)?;
            }
            VaultState::StashExternalGitRepo => {
                stash_external_git_repo(root, master)?;
            }
            VaultState::DeleteSealedGitFiles => {
                delete_sealed_git_files(root, master)?;
            },
            VaultState::ExpandMainVault => {
                expand_decrypted_bin(root.path(), master.decrypted_zip_bytes.take().ok_or_else(|| anyhow!("Could not get bytes."))?)?;
            }
            VaultState::ExpandLocalVault => {
                if !master.skip_local_zip {
                    expand_decrypted_bin(root.path(), master.decrypted_local_bytes.take().ok_or_else(|| anyhow!("Could not get bytes."))?)?;
                }
                
            }
            VaultState::CleanupOldBinaries => {
                cleanup_old_binaries(root)?;
            },
            VaultState::RestoreUnsecureFiles => {
                relocate_unsecure_files(root)?;
            }
            VaultState::Unsealed => {}

        }
        master.handle.writeback()?;

        Ok(())
    }
}

fn mark_init_done(ctx: &mut Context) -> Result<()> {


    ctx.handle.set_init(false);

    Ok(())
}

fn relocate_unsecure_files(root: &RootPath<Normal>) -> Result<()> {
    // let path = root.as_ref();

    if !root.unsecure_folder().exists() {
        // Sometimes there is nothing to move.
        return Ok(());
    }

    let unsecure_path = root.unsecure_folder();

    for dir in WalkDir::new(&unsecure_path) {
        let dir = dir?;
        if dir.depth() == 0 {
            continue;
        }
        let lpath = dir.path();
        let name = lpath.strip_prefix(&unsecure_path)?;

        if !lpath.exists() {
            // TODO: How do we want to handle this case?
        } else {
            std::fs::rename(lpath, root.path().join(name))?;
        }
    }

    // Now we delete the directory.
    std::fs::remove_dir_all(unsecure_path)?;

    Ok(())
}


fn cleanup_old_binaries(root: &RootPath<Normal>) -> Result<()> {
     // Remove the ignore and attributes files.
     std::fs::remove_file(root.vault_binary())?;

    // Remove the locally secured files.
    if root.secure_local_folder().exists() {
        std::fs::remove_dir_all(root.secure_local_folder())?;
    }
    

    Ok(())
}


fn expand_decrypted_bin(path: &Path, vault: Vec<u8>) -> Result<()> {
    let mut real = ZipArchive::new(Cursor::new(vault))?;

    for i in 0..real.len() {
        let mut entry = real.by_index(i)?;

        let rel_path = match entry.enclosed_name() {
            Some(p) => p.to_owned(),
            None => continue,
        };

        let out_path = path.join(rel_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .inspect_err(|e| {
                    console_log!(Error, "Failed creating a directory (path={out_path:?}): {e:?}");
                })?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).inspect_err(|e| {
                    console_log!(Error, "Failed creating a parent directory (path={parent:?}): {e:?}");
                })?;
            }

            let mut outfile = File::create(&out_path)
                .inspect_err(|e| {
                    console_log!(Error, "Failed creating a file (path={out_path:?}): {e:?}");
                })?;
            std::io::copy(&mut entry, &mut outfile)
                .inspect_err(|err| {
                    console_log!(Error, "Failed copying from buffer to {outfile:?} with error: {err:?}");
                })?;
        }
    }
    Ok(())
}

fn delete_sealed_git_files(root: &RootPath<Normal>, ctx: &mut Context) -> Result<()> {
    if ctx.handle.get_remote_storage()? == Some(SyncMethod::TigrisS3) {
        return Ok(());
    }
     // Remove the ignore and attributes files.
    std::fs::remove_file(root.gitignore())?;
    std::fs::remove_file(root.gitattributes())?;
    Ok(())
}

fn stash_external_git_repo(root: &RootPath<Normal>, context: &mut Context) -> Result<()> {
    if context.handle.get_remote_storage()? == Some(SyncMethod::TigrisS3) {
        return Ok(());
    }
    
    // If decryption passes we can start doing mutable ops.
    // Create the wrap directory.
    if !root.wrap_folder().exists() {
        std::fs::create_dir_all(root.wrap_folder())?;
    }

    // Move it into the wrap directory.
    std::fs::rename(root.local_git(), root.external_git())?;


    Ok(())
}

fn decrypt_zip(vault_path: &Path, master: &MasterVaultKey) -> Result<Vec<u8>> {
     let mut header = std::fs::read(vault_path)?;

    let mut vault = header.split_off(32);

    decrypt(&mut header, &mut vault, master.key_bytes())?;
    Ok(vault)
}

fn decrypt_main_vault(root: &RootPath<Normal>, master: &mut Context) -> Result<()> {

    let master_key = master.handle.get_wrapped_key()?;
    master.new_wrapped = Some(master_key.clone());

    let master_key = master_key.get_master_key_with_no_rewrap(master.password)?;

    master.master = Some(master_key.clone());

    master.decrypted_zip_bytes = Some(decrypt_zip(&root.vault_binary(), &master_key)?);


    Ok(())
}

fn decrypt_local_vault(root: &RootPath<Normal>, master: &mut Context) -> Result<()> {
    match master.master.clone() {
        Some(k) => {


            if !root.secure_local_zip().exists() {
                master.skip_local_zip = true;
            } else {
                master.decrypted_local_bytes = Some(decrypt_zip(&root.secure_local_zip(), &k)?);
            }
            

            Ok(())
        }
        None => Err(anyhow!("There was no master key installed, which usually means that the local vault decryption was run prior to the main vault decryption."))
    }
}

#[cfg(windows)]
fn to_wide(s: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}

fn make_external_git_repo(path: &RootPath<Normal>) -> Result<()> {
    make_git_repo(path.path())?;
    Ok(())
}

fn init_filesystem(path: &RootPath<Normal>) -> Result<()> {
    let met_path = path.metadata_folder();


    // if met_path.exists() {
    //     return Err(anyhow!("There was an existing metadata directory during initialization."));
    // }

      if !exists_git_repo(path.path()) {
            console_log!(Info, "Initializing a new git repository.");
            make_git_repo(path.path())?;
        } else {
            console_log!(Info, "There is already an existing git repository.");
        }


    if !met_path.exists() {
        // Create the metadata directory.
        std::fs::create_dir_all(&met_path)?;
    }
    

    #[cfg(windows)]
    {
        // On Windows you need special logic to make a directory properly hidden.
        use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, SetFileAttributesW};

        
        // Mark the file as hidden.
        if unsafe { SetFileAttributesW(to_wide(met_path.as_os_str()).as_ptr(), FILE_ATTRIBUTE_HIDDEN) } == 0 {
            use windows_sys::Win32::Foundation::GetLastError;

            let err = unsafe { GetLastError() };
            Err(std::io::Error::from_raw_os_error(err as i32))?;
        }
    }

    let ig_path = path.gitignore();
    if !ig_path.exists() {
        std::fs::write(&ig_path, "# Feel free to customize.\n\n# Leave the next line be.\n/.nov\n")?;
    }

    let toml = path.config();
    if !toml.exists() {
        std::fs::write(&toml, "[settings]\ndefault_policy = \"IgnoreAndEncrypt\"\n\n[rules]\nunsecured = []\ndelete = []\n".as_bytes())?;
    }


    Ok(())
}

fn seed_state(ctx: &mut Context) -> Result<()> {

    let master = MasterVaultKey::generate();
    let wrapped =  WrappedKey::init(&UserVaultKey::init_fresh(ctx.password)?, &master)?;

    ctx.handle.set_master_key(&wrapped);
    ctx.handle.set_init(true);

    Ok(())
}



fn recreate_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        // Create the folder.
        std::fs::create_dir_all(path)?;
    } else {
        std::fs::remove_dir_all(path)?;
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}


/// Performs the restoration if we were interrupted while encrypting.
/// 
/// In this case, we really have no choice but to delete all the directories,
/// as it is not safe to proceed with sealing unless we have already written
/// the new wrapped key.
fn revert_encryption_state(
    root: &RootPath<Normal>,
    ctx: &mut Context
) -> Result<()> {

    if root.deletion_shards().exists() {
        // If we have deletion shards, those will need to be cleaned up.
        std::fs::remove_file(root.deletion_shards())?;
    }

    if root.secure_local_zip().exists() {
        std::fs::remove_file(root.secure_local_zip())?;
    }

    if root.inprogress_vault().exists() {
        std::fs::remove_file(root.inprogress_vault())?;
    }

    // Now we can move ourselves to the unsealed state.
    // TODO: What about the case where encryption fails during init?
    if ctx.handle.get_init()? {
        ctx.handle.set_state(VaultState::RecreatingDirectories);
    } else {
        ctx.handle.set_state(VaultState::Unsealed);
    }
    

    Ok(())
}

fn write_encrypted_archives(
    root: &RootPath<Normal>,
    ctx: &mut Context
) -> Result<()> {

    let (new_wrap, master) = ctx.handle.get_wrapped_key()?.get_master_key(ctx.password)?;

    ctx.new_wrapped = Some(new_wrap.clone());

     let mut sec_local_writer = VaultWriter::new(root.secure_local_zip(), master.key_bytes())?;

    let mut enc_writer = VaultWriter::new(root.inprogress_vault(), master.key_bytes())?;

    let filter = NovFilter::from_root(root.path())?;


    

    let src_dir = root.canonicalize()?;

    let walk = walkdir::WalkDir::new(src_dir.path());

    let mut to_unlink = vec![];

    


    for file in walk.into_iter().filter_entry(|e| e.file_name() != ".nov") {
        let file = file?;
        if file.depth() == 0 {
            continue;
        }
        let path = file.path();
        let name = path.strip_prefix(src_dir.path())?;

    
        // Schedule this path for unlinking.
        to_unlink.push(file.clone());
        match filter.check_decision(path)? {
            FilterDecision::Delete => {
   
                // We do not do anything and allow it to unlink.
            }
            FilterDecision::Encrypt => {
                // We just write it to the normal zip.
                enc_writer.write_path(path, name)?;
                // write_path_to_zip(&mut enc_zip, enc_options, path, name)?;
            }
            FilterDecision::IgnoreAndEncrypt => {
                // println!("IGNORE AND ENCRYPT: {:?}", path);
                sec_local_writer.write_path(path, name)?;
            }
            FilterDecision::Unsecure => {
                if path.is_dir() {
                    std::fs::create_dir_all(root.unsecure_folder().join(name))?;
                } else {
                    std::fs::write(root.unsecure_folder().join(name), std::fs::read(path)?)?;
                }
            } // }
        }
    }



    let mut buf = BufWriter::new(File::create(root.deletion_shards())?);
    

    for link in to_unlink {
        // Write the path.
        buf.write_all(link.path().to_string_lossy().as_bytes())?;
        buf.write_all(b"\n")?;
    }

    // Flush the buffer to the disk.
    buf.flush()?;
    


    enc_writer.finish()?;
    sec_local_writer.finish()?;

    ctx.handle.set_master_key(&new_wrap);
    

    Ok(())
}


/// Unlinks the artifacts.
fn unlink_other_archives(root: &RootPath<Normal>) -> Result<()> {
    let delete = root.deletion_shards();
    if !delete.exists() {
        return Err(anyhow!("Failed to find the deletion shards achive, could not proceed with unlinking."));
    }


    let bufr = BufReader::new(File::open(root.deletion_shards())?);

    for line in bufr.lines() {
        let line = line?;
        if line.is_empty() {
            continue; // We do not want to interact with empty lines.
        }
        let path = PathBuf::from_str(&line)?;
        if path.exists() {
            // Unlink the file.
            if path.is_dir() {
                std::fs::remove_dir_all(&path)
                    .map_err(|e| anyhow!("Failed unlinking directory (path={path:?}) with error {e:?}"))?;
            } else {
                 std::fs::remove_file(&path).map_err(|e| anyhow!("Failed unlinking file (path={path:?}) with error {e:?}"))?;
            }
           
        }
    }

    // Remove the deletion shards.
    std::fs::remove_file(root.deletion_shards())?;


    Ok(())
}

fn relocate_encrypted_binaries(root: &RootPath<Normal>) -> Result<()> {
    std::fs::rename(root.inprogress_vault(), root.vault_binary())?;
    Ok(())
}

fn recreate_directories(root: &RootPath<Normal>) -> Result<()> {
    // Recreate the folders.
    recreate_dir(&root.unsecure_folder())?;
    recreate_dir(&root.secure_local_folder())?;
    

    if root.deletion_shards().exists() {
        std::fs::remove_file(root.deletion_shards())?;
    }

    // Delete the in progress zip.
    let zip_loc = root.inprogress_vault();
    if zip_loc.exists() {
        std::fs::remove_file(&zip_loc)?;
    }
    Ok(())
}

fn create_mandatory_post_seal_files(root: &RootPath<Normal>) -> Result<()> {
    std::fs::write(
        root.gitignore(),
        "# NOVAULT\n# DO NOT MODIFY THIS\n/.nov/unsecure\n/.nov/secure_local\n/.nov/.s3auth\n",
    )?;
    std::fs::write(
        root.gitattributes(),
        "# NOVAULT\n# DO NOT MODIFY THIS\nvault.bin binary",
    )?;
    Ok(())
}

fn restore_vault_git(root: &RootPath<Normal>, ctx: &mut Context) -> Result<()> {
    if ctx.handle.get_remote_storage()? == Some(SyncMethod::TigrisS3) {
        return Ok(());
    }
    // Now we need to move the .git back out to the top.
    std::fs::rename(
        root.external_git(),
        root.local_git(),
    )?;

    // Remove the wrap directory as it serves no purpose when we are sealed.
    std::fs::remove_dir(root.wrap_folder())?;

    Ok(())
}
