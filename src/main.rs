#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod action;
pub mod agent;
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod mode;
pub mod styles;
pub mod tui;
pub mod utils;

use clap::Parser;
use cli::Cli;
use color_eyre::eyre::Result;
use replicate_rs::config::ReplicateConfig;

use crate::{
    app::App,
    utils::{initialize_logging, initialize_panic_handler, version},
};

async fn tokio_main() -> Result<()> {
    initialize_logging()?;

    initialize_panic_handler()?;

    let args = Cli::parse();

    let config = ReplicateConfig::new();
    match config {
        Ok(..) => {
            let mut app = App::new(args.tick_rate, args.frame_rate)?;
            app.run().await?;
        }
        Err(err) => {
            eprintln!("{err}");
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = tokio_main().await {
        eprintln!("{} error: Something went wrong", env!("CARGO_PKG_NAME"));
        Err(e)
    } else {
        Ok(())
    }
}
