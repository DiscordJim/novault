
use clap::Parser;

use crate::{
    cli::Args,
    sys::{
        common::{link, open, pull, seal_full, sync, unseal},
        init::run_init, lib::sync::load_tigris_params,
    },
};
use anyhow::Result;

mod cli;
mod printing;
mod sys;


fn run_subcommand() -> Result<()> {
    let args = Args::parse();
    match args {
        Args::Init { target } => run_init(target),
        Args::Seal { target } => seal_full(target),
        Args::Unseal { target } => unseal(target),
        Args::Sync { target } => sync(target),
        Args::Link { target, url } => link(target, &url),
        Args::Pull { target, url } => pull(target, &url),
        Args::Open { target } => open(target)
    }
}

fn main() {
   
    match run_subcommand() {
        Ok(()) => {}
        Err(e) => {
            console_log!(Error, "{e:?}");
        }
    }
}
