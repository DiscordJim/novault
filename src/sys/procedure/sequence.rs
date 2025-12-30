use crate::sys::{lib::path::{Normal, RootPath}, procedure::actions::{Context, VaultState}};

/// Preinitializes the VAULT.
pub const PRE_INIT: &[VaultState] = &[VaultState::InitFileSystem, VaultState::Seed];

pub const SEAL_SEQUENCE_PARTIAL: &[VaultState] = &[
    VaultState::RecreatingDirectories,
    VaultState::Encrypting,
    VaultState::UnlinkPostSeal,
    VaultState::RelocateEncryptedBinaries,
    VaultState::WriteMandatoryPostSealFiles
];


pub const SEAL_FULL: &[&[VaultState]] = &[
    &SEAL_SEQUENCE_PARTIAL,
    &[ VaultState::RestoreVaultGit, VaultState::Sealed ]
];

pub const INIT_FULL: &[&[VaultState]] = &[
    &PRE_INIT,
    &SEAL_SEQUENCE_PARTIAL,
    &[ VaultState::MakeExternalGitRepo, VaultState::Sealed ]
];

pub const UNSEAL_FULL: &[VaultState] = &[
    VaultState::DecryptMainVault,
    VaultState::DecryptLocallySecuredVault,
    VaultState::StashExternalGitRepo,
    VaultState::DeleteSealedGitFiles,
    VaultState::ExpandMainVault,
    VaultState::ExpandLocalVault,
    VaultState::CleanupOldBinaries,
    VaultState::RestoreUnsecureFiles,
    VaultState::Unsealed
];



pub trait Playable {
    fn into_iter(&self) -> impl Iterator<Item = VaultState>;
    fn play(&self, root: &RootPath<Normal>, ctx: &mut Context) -> anyhow::Result<()> {
        for order in self.into_iter() {
            order.act(root, ctx)?;
        }
        Ok(())
    }
}

impl Playable for &[VaultState] {
    fn into_iter(&self) -> impl Iterator<Item = VaultState> {
        self.iter().copied()
    }
}

impl Playable for &[&[VaultState]] {
    fn into_iter(&self) -> impl Iterator<Item = VaultState> {
        // let mut iter: Option<Box<dyn Iterator<Item = VaultState>>> = None;

        self.iter().map(|f| f.iter()).flatten().copied()
        
    }
}