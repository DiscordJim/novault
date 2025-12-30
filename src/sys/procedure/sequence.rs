use crate::sys::{lib::path::{Normal, RootPath}, procedure::actions::{Context, VaultState}};

/// Preinitializes the VAULT.
pub const PRE_INIT: NormalSequence = NormalSequence(&[VaultState::InitFileSystem, VaultState::Seed]);

pub const SEAL_SEQUENCE_PARTIAL: NormalSequence = NormalSequence(&[
    VaultState::RecreatingDirectories,
    VaultState::Encrypting,
    VaultState::UnlinkPostSeal,
    VaultState::RelocateEncryptedBinaries,
    VaultState::WriteMandatoryPostSealFiles
]);

#[derive(Clone)]
pub struct NormalSequence(&'static [VaultState]);

#[derive(Clone)]
pub struct ComposedSequence(&'static [NormalSequence]);


pub const SEAL_FULL: ComposedSequence = ComposedSequence(&[
    SEAL_SEQUENCE_PARTIAL,
    NormalSequence(&[ VaultState::RestoreVaultGit, VaultState::Sealed ])
]);

pub const INIT_FULL: ComposedSequence = ComposedSequence(&[
    PRE_INIT,
    SEAL_SEQUENCE_PARTIAL,
    NormalSequence(&[ VaultState::MakeExternalGitRepo, VaultState::MarkInitDone, VaultState::Sealed ])
]);

pub const UNSEAL_FULL: NormalSequence = NormalSequence(&[
    VaultState::DecryptMainVault,
    VaultState::DecryptLocallySecuredVault,
    VaultState::StashExternalGitRepo,
    VaultState::DeleteSealedGitFiles,
    VaultState::ExpandMainVault,
    VaultState::ExpandLocalVault,
    VaultState::CleanupOldBinaries,
    VaultState::RestoreUnsecureFiles,
    VaultState::Unsealed
]);



pub trait Playable: Clone {
    fn into_iter(&self) -> impl Iterator<Item = VaultState> + Clone;
    fn play(&self, root: &RootPath<Normal>, ctx: &mut Context) -> anyhow::Result<()> {
        for order in self.into_iter() {
            order.act(root, ctx)?;
        }
        Ok(())
    }
    fn resume(&self, pos: VaultState) -> impl Playable {
        self.into_iter().skip_while(move |x| *x != pos)
    }
}

impl<D> Playable for D
where 
    D: Iterator<Item = VaultState> + Clone
{
    fn into_iter(&self) -> impl Iterator<Item = VaultState> + Clone {
        // self.into_iter()
        self.clone()
    }



}

impl Playable for NormalSequence {
    fn into_iter(&self) -> impl Iterator<Item = VaultState> + Clone {
        self.0.iter().copied()
    }
}

impl Playable for ComposedSequence {
    fn into_iter(&self) -> impl Iterator<Item = VaultState> + Clone {
        // let mut iter: Option<Box<dyn Iterator<Item = VaultState>>> = None;

        self.0.iter().map(|f| f.0.iter()).flatten().copied()
        
    }
}