#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod action;
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

async fn tokio_main() -> anyhow::Result<()> {
    initialize_logging()?;

    initialize_panic_handler()?;

    let args = Cli::parse();

    if std::env::var("REPLICATE_API_KEY").is_ok() || std::env::var("TOGETHER_API_KEY").is_ok() {
        let mut app = App::new(args.tick_rate, args.frame_rate)?;
        app.run().await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(e) = tokio_main().await {
        eprintln!("{} error: Something went wrong", env!("CARGO_PKG_NAME"));
        Err(e)
    } else {
        Ok(())
    }
}
