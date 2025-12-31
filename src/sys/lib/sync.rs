use std::{io::Write, path::Path};

use anyhow::{Result, anyhow};
use colorize::AnsiColor;

use crate::{
    console_log,
    sys::{
        common::prompt_s3_access_key_and_pass, lib::{
            path::{Normal, RootPath},
            remote::t3::{get_snapshot_sig, t3_delete, t3_fetch, t3_put},
        }, process::{add_remote_origin, git_add_commit_push, git_branch_main, git_clone}, statefile::{StateFileHandle, SyncMethod, read_hashmap, string_to_hashmap}
    },
};

fn parse_link(url: &str) -> Result<String> {
    // Form 1: git@github.com:JohnSmith/vault.git
    if url.starts_with("git@") {
        Ok(url.to_string())
    } else {
        Err(anyhow!(
            "We only support SSH style URLs at this time: git@github:JohnSmith/vault.git"
        ))
    }
}

pub fn init_remote(path: &Path, url: &str) -> Result<()> {
    let (sync, url) = if url.starts_with("t3://") {
        (SyncMethod::TigrisS3, url[5..].to_string())
    } else if url.starts_with("git@") {
        (SyncMethod::Git, url.to_string())
    } else {
        return Err(anyhow!("Could not match remote."));
    };

    match sync {
        SyncMethod::Git => {
            let url = parse_link(&url)?;

            console_log!(Info, "Adding {url} as a remote origin...");
            add_remote_origin(path, &url)?;

            console_log!(Info, "Switching branch to main...");
            git_branch_main(path)?;

            console_log!(Info, "Pushing initial commit to main...");
            git_add_commit_push(path)?;

            let mut handle = StateFileHandle::new(path)?;
            handle.set_remote(&url);
            handle.set_remote_storage(SyncMethod::Git);
            handle.writeback()?;

            console_log!(Info, "Succesfully linked to branch to remote!");
        }
        SyncMethod::TigrisS3 => {
            let snapshot = get_snapshot_sig();
            let rem_s3 = Path::new(&snapshot);

            let (s3_access, s3_secret) = load_tigris_params(&RootPath::new(path))?;

            console_log!(Info, "Sending files to remote...");
            print!("  {} Sending state dictionary...", "(1/3)".yellow());
            std::io::stdout().flush()?;

            transfer_file(
                &s3_access,
                &s3_secret,
                &url,
                path,
                &path.join(".nov").join(".state"),
                &rem_s3,
            )?;

            print!(
                "\r  {} State dictionary uploaded.              ",
                "(1/3)".green()
            );
            std::io::stdout().flush()?;

            print!("\n  {} Sending vault binary...", "(2/3)".yellow());
            std::io::stdout().flush()?;

            transfer_file(
                &s3_access,
                &s3_secret,
                &url,
                path,
                &path.join("vault.bin"),
                &rem_s3,
            )?;

            print!("\r  {} Sent vault binary.           \n", "(2/3)".green());

            print!("  {} Setting remote lock...", "(3/3)".yellow());
            std::io::stdout().flush()?;

            set_lock(&s3_access, &s3_secret, &url, &snapshot)?;

            print!("\r  {} Set remote lock.           \n", "(3/3)".green());

            // std::io::stdout().flush()?;

            console_log!(Info, "Sent files to remote succesfully.");

            let mut handle = StateFileHandle::new(path)?;
            handle.set_remote(&url);
            handle.set_remote_storage(SyncMethod::TigrisS3);
            handle.previous_tigris_commit_stamp(&snapshot);
            handle.writeback()?;

            console_log!(Info, "Configured the storage backend to be TigrisS3.");
        }
    }

    Ok(())
}

pub fn set_lock(
    s3_access: &str,
    s3_secret: &str,
    remote_parameters: &str,
    stamp: &str,
) -> Result<()> {
    t3_put(
        s3_access,
        s3_secret,
        remote_parameters,
        Path::new(".lock"),
        stamp.as_bytes().to_vec(),
    )?;

    Ok(())
}

pub fn transfer_file(
    s3_access: &str,
    s3_secret: &str,
    remote_parameters: &str,
    root: &Path,
    location: &Path,
    remote_root: &Path,
) -> Result<()> {
    let bytes = std::fs::read(location)?;

    t3_put(
        s3_access,
        s3_secret,
        remote_parameters,
        &remote_root.join(location.strip_prefix(root)?),
        bytes,
    )?;

    Ok(())
}

fn load_s3_params_direct(path: &RootPath<Normal>) -> Result<(String, String)> {

    let load = string_to_hashmap(&std::fs::read_to_string(path.s3_param_file())?);

    let access = load.get("ACCESS_KEY").ok_or_else(|| anyhow!("Failed to read the access key."))?;
    let secret = load.get("SECRET_KEY").ok_or_else(|| anyhow!("Failed to get the secret key."))?;

    Ok((access.to_string(), secret.to_string()))
}

pub fn load_tigris_params(path: &RootPath<Normal>) -> Result<(String, String)> {
    if path.s3_param_file().exists() {
        console_log!(Info, "Found Tigris parameters, attempting to load them.");
        if let Ok((acc, sec)) = load_s3_params_direct(path).inspect_err(|e| {
            console_log!(Error, "Failed to read Tigris S3 parameters: {e:?}");
        }) {
            return Ok((acc, sec));
        }
        console_log!(Warn, "There was an error reading the parameters, they will need to be recreated.");
        std::fs::remove_file(path.s3_param_file())?;
    }

    let (acc, sec) = prompt_s3_access_key_and_pass()?;
    std::fs::write(path.s3_param_file(), format!("ACCESS_KEY={acc}\nSECRET_KEY={sec}\n"))?;

    // TODO: Test the actual parameters to see if they are right.

    Ok((acc, sec))

    // t3_fetch(&acc, &sec,, file_path)
}

pub fn pull_remote(path: &Path, url: &str) -> Result<()> {
    let (sync, url) = if url.starts_with("t3://") {
        (SyncMethod::TigrisS3, url[5..].to_string())
    } else if url.starts_with("git@") {
        (SyncMethod::Git, url.to_string())
    } else {
        return Err(anyhow!("Could not match remote."));
    };

    match sync {
        SyncMethod::Git => {
            git_clone(path, &url)?;
        }
        SyncMethod::TigrisS3 => {

            console_log!(Info, "NovoVault will use TigrisT3 as the backend.");
            

            let (access, secret) = prompt_s3_access_key_and_pass()?;

            console_log!(Info, "(TigrisT3) Pulling from bucket {url}...");

            // console_log!(Info, "Determined latest commit to be {string}...");
            print!("  {} Finding latest commit...", "(1/3)".yellow());
            std::io::stdout().flush()?;

            let lock = t3_fetch(&access, &secret, &url, Path::new(".lock"))?;
            let string = std::str::from_utf8(&lock)?;

            print!(
                "\r  {} Found latest commit ({string}).              ",
                "(1/3)".green()
            );

            let rem_root = Path::new(string);

            print!("\n  {} Pulling state file...", "(2/3)".yellow());
            std::io::stdout().flush()?;

            let state_file = t3_fetch(
                &access,
                &secret,
                &url,
                &rem_root.join(".nov").join(".state"),
            )?;

            print!("\r  {} Pulled state file.               ", "(2/3)".green());

            print!("\n  {} Pulling vault binary...", "(3/3)".yellow());
            std::io::stdout().flush()?;

            let vault_bin = t3_fetch(&access, &secret, &url, &rem_root.join("vault.bin"))?;

            print!(
                "\r  {} Pulled vault binary.               \n",
                "(3/3)".green()
            );

            console_log!(
                Info,
                "(TigrisT3) Finished pulling artifacts. Now will write them to disk."
            );

            let path = RootPath::new(path);

            if !path.metadata_folder().exists() {
                std::fs::create_dir_all(path.metadata_folder())?;
            }

            std::fs::write(path.s3_param_file(), format!("ACCESS_KEY={access}\nSECRET_KEY={secret}\n"))?;

            std::fs::write(path.state_file(), state_file)?;
            std::fs::write(path.vault_binary(), vault_bin)?;

            console_log!(Info, "(TigrisT3) Wrote artifacts to disk.");
        }
    }

    Ok(())
}

pub fn push_remote(path: &Path) -> Result<()> {
    // println!("Hello 4.1");
    let path = RootPath::new(path);

    let mut state = StateFileHandle::new(path.path())?;

    let Some(sync) = state.get_remote_storage()? else {
        return Err(anyhow!("We currently have no sync method configured."));
    };

    // println!("hello 4.2");

    match sync {
        SyncMethod::Git => {
            console_log!(Info, "Performing synchronization...");
            git_add_commit_push(path.path())?;
            console_log!(Info, "Synchronization complete...");
        }
        SyncMethod::TigrisS3 => {
            let bucket = state
                .get_remote()
                .ok_or_else(|| anyhow!("We have TigrisT3 but no remote set?"))?;
            let last_commit = state.get_previous_tigris_commit_stamp()?;


            // println!("Hello 4.3");
            let (s3_access, s3_secret) = load_tigris_params(&path)?;

            // println!("Hello 4.4");

            console_log!(Info, "Sending files to remote...");
            print!("  {} Sending state dictionary...", "(1/3)".yellow());
            std::io::stdout().flush()?;

            let snapshot = get_snapshot_sig();
            let rem_s3 = Path::new(&snapshot);

            transfer_file(
                &s3_access,
                &s3_secret,
                &bucket,
                path.path(),
                &path.state_file(),
                &rem_s3,
            )?;

            print!(
                "\r  {} State dictionary uploaded.              ",
                "(1/3)".green()
            );
            std::io::stdout().flush()?;

            print!("\n  {} Sending vault binary...", "(2/3)".yellow());
            std::io::stdout().flush()?;

            transfer_file(
                &s3_access,
                &s3_secret,
                &bucket,
                path.path(),
                &path.vault_binary(),
                &rem_s3,
            )?;

            print!("\r  {} Sent vault binary.           \n", "(2/4)".green());

            print!("  {} Setting remote lock...", "(3/3)".yellow());
            std::io::stdout().flush()?;

            set_lock(&s3_access, &s3_secret, &bucket, &snapshot)?;

            print!("\r  {} Set remote lock.           \n", "(3/4)".green());

            print!("  {} Deleting old repo...", "(3/4)".yellow());
            std::io::stdout().flush()?;

            t3_delete(
                &s3_access,
                &s3_secret,
                &bucket,
                &Path::new(&last_commit).join("vault.bin"),
            )?;
            t3_delete(
                &s3_access,
                &s3_secret,
                &bucket,
                &Path::new(&last_commit).join(".nov").join(".state"),
            )?;

            print!("\r  {} Deleted old folder.           \n", "(4/4)".green());
        
            state.previous_tigris_commit_stamp(&snapshot);
            state.writeback()?;
        
        }
    }

    Ok(())
}
