use anyhow::{Result, anyhow};
use std::{fs::File, io::{BufWriter, Write}, path::Path};

use crate::sys::{filter::{FilterDecision, NovFilter}, lib::path::{Normal, RootPath}, mk::MasterVaultKey, writer::VaultWriter};

pub enum VaultState {
    RecreatingDirectories,
    Encrypting,
    WriteArchives,
    UnlinkPostSeal,
    RelocateEncryptedBinaries,
    WriteMandatoryPostSealFiles,
}

impl VaultState {
    pub fn act(&self, root: &RootPath<Normal>) -> Result<()> {
        match self {
            VaultState::RecreatingDirectories => {
                recreate_directories(root)?;
            }
            VaultState::Encrypting => {}
            VaultState::WriteArchives => {}
            VaultState::UnlinkPostSeal => {}
            VaultState::RelocateEncryptedBinaries => {
                relocate_encrypted_binaries(root)?;
            }
            VaultState::WriteMandatoryPostSealFiles => {
                create_mandatory_post_seal_files(root)?;
            }
        }

        Ok(())
    }
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

fn write_encrypted_archives(
    root: &RootPath<Normal>,
    master: &MasterVaultKey
) -> Result<()> {

     let mut sec_local_writer = VaultWriter::new(root.secure_local_zip(), master.key_bytes())?;

    let mut enc_writer = VaultWriter::new(root.inprogress_vault(), &master.key_bytes())?;

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
    

    // Flush the buffer to the disk.
    buf.flush()?;
    


    enc_writer.finish()?;
    sec_local_writer.finish()?;
    Ok(())
}

fn relocate_encrypted_binaries(root: &RootPath<Normal>) -> Result<()> {
    std::fs::rename(&root.inprogress_vault(), &root.vault_binary())?;
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
        "# NOVAULT\n# DO NOT MODIFY THIS\n/.nov/unsecure\n/.nov/secure_local",
    )?;
    std::fs::write(
        root.gitattributes(),
        "# NOVAULT\n# DO NOT MODIFY THIS\nvault.bin binary",
    )?;
    Ok(())
}
