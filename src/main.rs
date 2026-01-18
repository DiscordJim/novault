
use std::{process::exit, time::Duration};

use clap::Parser;

use crate::{
    cli::Args, printing::SteppedComputationHandle, sys::{
        common::{link, open, pull, seal_full, sync, unseal},
        init::run_init, lib::sync::load_tigris_params,
    }
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


    let mut comp = SteppedComputationHandle::start("Yeeee", 4);
    comp.start_next("Hello", "Bye", || {
        
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }).unwrap();

    comp.start_next("Hello 2", "Bye 2", || {
        
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }).unwrap();

    // START SUB
    {
        
    let mut comp2 = SteppedComputationHandle::start("Sus", 3);
    comp2.start_next("Hello", "Bye", || {
        
        std::thread::sleep(Duration::from_secs(3));
        Ok(())
    }).unwrap();

    comp2.start_next("Hello 2", "Bye 2", || {
        
        std::thread::sleep(Duration::from_secs(3));
        Ok(())
    }).unwrap();

    // START SUB
    {
        
    let mut comp2 = SteppedComputationHandle::start("Sus2", 3);
    comp2.start_next("Hello", "Bye", || {
        
        std::thread::sleep(Duration::from_secs(3));
        Ok(())
    }).unwrap();

    comp2.start_next("Hello 2", "Bye 2", || {
        
        std::thread::sleep(Duration::from_secs(3));
        Ok(())
    }).unwrap();

    comp2.start_next("Hello 3", "Bye 3", || {
        
        std::thread::sleep(Duration::from_secs(3));
        Ok(())
    }).unwrap();

    comp2.finish();
    }

    comp2.start_next("Hello 3", "Bye 3", || {
        
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }).unwrap();

    comp2.finish();
    }

    // END SUB

    comp.start_next("Hello 3", "Bye 3", || {
        
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }).unwrap();

    

    comp.start_next("Hello 4", "Bye 4", || {
        
        std::thread::sleep(Duration::from_secs(2));
        Ok(())
    }).unwrap();

    comp.finish();

    std::thread::sleep(Duration::from_secs(2));


    if 1 + 1 == 2 {
        exit(1);
    }

   
    match run_subcommand() {
        Ok(()) => {}
        Err(e) => {
            console_log!(Error, "{e:?}");
        }
    }
}
