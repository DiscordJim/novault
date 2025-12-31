use anyhow::{Result, anyhow};
use colorize::AnsiColor;
use crossterm::{
    event::{Event, KeyCode, KeyModifiers, read},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::{
    env,
    io::{Write, stdout},
    path::Path,
};

use crate::{
    console_log,
    sys::{
        lib::path::{Normal, RootPath},
        mk::{CachedPassword, WrappedKey},
        procedure::{
            actions::{Context, VaultState},
            sequence::{Playable, SEAL_FULL, UNSEAL_FULL},
        },
        process::{add_remote_origin, git_add_commit_push, git_branch_main, git_clone},
        statefile::StateFileHandle,
    },
};

/// Checks if a git repo exists at the target location.
pub fn exists_git_repo(root: impl AsRef<Path>) -> bool {
    root.as_ref().join(".git").exists()
}

/// Creates a Git repository at the location.
pub fn make_git_repo(root: impl AsRef<Path>) -> Result<()> {
    use std::process;
    process::Command::new("git")
        .args(vec![
            "init".to_string(),
            root.as_ref().to_str().unwrap().to_string(),
        ])
        .output()?;
    Ok(())
}

pub fn seal_full(root: impl AsRef<Path>) -> Result<()> {
    let path = root.as_ref();

    if let Ok(mut sfh) = StateFileHandle::new(path)
        && let Ok(VaultState::Sealed) = sfh.get_state()
    {
        console_log!(Info, "The vault is already sealed.");
        return Ok(());
    }

    let mut usr_input = get_password_with_prompt(false)?;

    let root = RootPath::new(path);
    let mut ctx = Context::new(&root, &mut usr_input)?;
    SEAL_FULL.play(&root, &mut ctx)?;

    Ok(())
}

pub fn unseal(root: impl AsRef<Path>) -> Result<()> {
    if let Ok(mut sfh) = StateFileHandle::new(root.as_ref()) && let Ok(VaultState::Unsealed) = sfh.get_state() {
            console_log!(Info, "The vault is already unsealed.");
            return Ok(());
        
    }

    let mut password = prompt_password(false)?;

    let root = RootPath::new(root.as_ref());
    let mut ctx = Context::new(&root, &mut password)?;

    UNSEAL_FULL.play(&root, &mut ctx)?;

    // Now we perform an unseal with the password.
    // unseal_with_pwd(path, &mut password)?;

    Ok(())
}

/// Performs a sync, which basically detects which mode
/// we are currently in.
pub fn sync(root: impl AsRef<Path>) -> Result<()> {
    require_seal(
        &RootPath::new(root.as_ref()),
        |sf| match sf.get_remote() {
            Some(_) => Ok(()),
            None => Err(anyhow!("The remote URL is not set. Please run link first.")),
        },
        || {
            console_log!(Info, "Performing synchronization...");
            git_add_commit_push(root.as_ref())?;
            Ok(())
        },
    )?;

    Ok(())
}

struct TermGuard;

impl TermGuard {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

/// This is a way
pub fn open(root: impl AsRef<Path>) -> Result<()> {
    let wrapped = StateFileHandle::new(root.as_ref())?.get_wrapped_key()?;
    let mut password = fetch_password(&wrapped)?;

    
    let mut context = Context::new(&RootPath::new(root.as_ref()), &mut password)?;

    UNSEAL_FULL.play(
        &RootPath::new(root.as_ref()),
        &mut context
    )?;

    

    // Opens the internals.
    let e = open_internal(&RootPath::new(root.as_ref()), &mut context);

    if StateFileHandle::new(root.as_ref())?.get_state()? == VaultState::Unsealed {
        SEAL_FULL.play(
            &RootPath::new(root.as_ref()),
            &mut context
        )?;
    }

    e
}

fn open_internal(root: &RootPath<Normal>, password: &mut Context) -> Result<()> {
    console_log!(Info, "The repository is open for editing.");
    console_log!(
        Info,
        "Commands:\n\t(Q) Quit and re-seal.\n\t(S) Synchronize"
    );

    let _guard = TermGuard::new();
    loop {
        if let Event::Key(key) = read()? {
            // Event::Key(key) => {
            if key.is_press() {
                if key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    break;
                }
                if key.code == KeyCode::Char('s') {
                    console_log!(Info, "Synchronizing repository with remote...");

                    require_seal_with_retrieval(
                        root,
                        |sf| match sf.get_remote() {
                            Some(_) => Ok(()),
                            None => {
                                Err(anyhow!("The remote URL is not set. Please run link first."))
                            }
                        },
                        |_| Ok(password.password().clone()),
                        || {
                            console_log!(Info, "Performing synchronization...");
                            git_add_commit_push(root.path())?;
                            Ok(())
                        },
                    )?;

                    console_log!(Info, "Synchronization complete...");
                }
            }
            // }
            // _ => {}
        }
    }
    // disable_raw_mode()?;
    drop(_guard);
    console_log!(Info, "Resealing the repository...");

    Ok(())
}

/// This is a guard that makes sure things happen in the correct order
/// and is used for a few operations that require the seal-unseal pattern.
fn require_seal<PF, F>(root: &RootPath<Normal>, pf_functor: PF, functor: F) -> Result<()>
where
    PF: FnMut(&StateFileHandle) -> Result<()>,
    F: FnMut() -> Result<()>,
{
    require_seal_with_retrieval(root, pf_functor, fetch_password, functor)
}

/// This is a guard that makes sure things happen in the correct order
/// and is used for a few operations that require the seal-unseal pattern.
fn require_seal_with_retrieval<PF, KR, F>(
    root: &RootPath<Normal>,
    mut pf_functor: PF,
    mut kr_functor: KR,
    mut functor: F,
) -> Result<()>
where
    PF: FnMut(&StateFileHandle) -> Result<()>,
    KR: FnMut(&WrappedKey) -> Result<CachedPassword>,
    F: FnMut() -> Result<()>,
{
    // let wrapped = state_file.get_mk()?;

    let mut state_file_handle = StateFileHandle::new(root.path())?;
    let wrapped = state_file_handle.get_wrapped_key()?;

    let mut password = kr_functor(&wrapped)?;

    let mut context = Context::new(root, &mut password)?;

    pf_functor(&state_file_handle)?;
    state_file_handle.writeback()?;

    if state_file_handle.get_state()? == VaultState::Unsealed {
        SEAL_FULL.play(root, &mut context)?;

        let e = functor();

        UNSEAL_FULL.play(root, &mut context)?;

        state_file_handle.writeback()?;

        e?
    } else {
        // We can run the functor immediately.
        functor()?;
    }

    Ok(())
}

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

pub fn link(root: impl AsRef<Path>, url: &str) -> Result<()> {
    let path = root.as_ref();

    // TODO: Check to see if the repository is well-formed.

    require_seal(
        &RootPath::new(path),
        |_| Ok(()),
        || {
            let url = parse_link(url)?;

            console_log!(Info, "Adding {url} as a remote origin...");
            add_remote_origin(path, &url)?;

            console_log!(Info, "Switching branch to main...");
            git_branch_main(path)?;

            console_log!(Info, "Pushing initial commit to main...");
            git_add_commit_push(path)?;

            let mut handle = StateFileHandle::new(root.as_ref())?;
            handle.set_remote(&url);
            handle.writeback()?;

            console_log!(Info, "Succesfully linked to branch to remote!");

            Ok(())
        },
    )?;

    Ok(())
}

pub fn pull(root: impl AsRef<Path>, url: &str) -> Result<()> {
    let url = parse_link(url)?;

    if RootPath::new(root.as_ref()).metadata_folder().exists() {
        return Err(anyhow!(
            "We can only pull to a repository that has not yet been initialized."
        ));
    }

    // if get_repo_state(root.as_ref())? != NovState::Uninit {
    //     return Err(anyhow!(
    //         "We cannot perform a pull unless the repository is uninitialized (i.e., an empty folder)."
    //     ));
    // }

    git_clone(root.as_ref(), &url)?;

    Ok(())
}

fn get_password_with_prompt(confirm: bool) -> Result<CachedPassword> {
    print!("{} ", "PROMPT".magenta().bold());
    stdout().flush()?;

    let scan = CachedPassword::from_string(rpassword::prompt_password(if !confirm {
        "Enter vault password: "
    } else {
        "Confirm password: "
    })?);
    Ok(scan)
}

/// Prompts for a password, optionally asking for
/// password confirmation.
///
/// Getting the password with confirmation is commonly
/// used when we are initializing a new vault.
pub fn prompt_password(confirm: bool) -> Result<CachedPassword> {
    let first = get_password_with_prompt(false)?;

    if confirm {
        let second = get_password_with_prompt(true)?;

        if first == second {
            Ok(first)
        } else {
            Err(anyhow!("Passowrds fail to match."))
        }
    } else {
        Ok(first)
    }
}

pub fn fetch_password(wrapped: &WrappedKey) -> Result<CachedPassword> {
    match env::var("novpwd").map(CachedPassword::from_string) {
        Ok(mut e) => {
            console_log!(
                Info,
                "Found a password in the shell variables, trying the password."
            );
            if wrapped.get_master_key(&mut e).is_ok() {
                console_log!(Info, "Password succesfully verified.");
                Ok(e)
            } else {
                console_log!(
                    Error,
                    "The password could not be verified and thus will need to be entered manually."
                );
                prompt_password(false)
            }
        }
        Err(_) => prompt_password(false),
    }
}
