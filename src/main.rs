#[macro_use]
extern crate log;

use std::cmp::min;

use anyhow::{anyhow, Result};
use async_ctrlc::CtrlC;
use async_std::prelude::FutureExt;
use env_logger::Env;
use tide::Request;
use clap::Parser;

/// Collect events from Homely API (REST and WebSockets) and send to MQTT
#[derive(Parser,Debug)]
#[clap(author="Morten Lied Johansen", version, about, long_about = None)]
struct Args {
    /// Listen address
    #[clap(short, long, default_value = "localhost:8080")]
    listen_address: String,

    /// Control verbosity of logs. Can be repeated
    #[clap(short, long, parse(from_occurrences))]
    verbose: usize,
}


#[async_std::main]
async fn main() -> Result<()> {
    let args: Args = Args::parse();

    init_logging(&args);

    let app = async {
        let mut app = tide::new();
        app.at("/healthz").get(probe);
        app.listen(args.listen_address)
            .await
            .map_err(|e| anyhow!(e))
    };

    app.race(setup_ctrlc_handler()).await?;

    Ok(())
}

async fn probe(mut _req: Request<()>) -> tide::Result {
    debug!("I'm being probed!");
    Ok(format!("I'm alive!").into())
}

/// ^C handler will stop the server
async fn setup_ctrlc_handler() -> Result<()> {
    CtrlC::new().expect("Cannot use CTRL-C handler").await;
    println!("termination signal received, stopping server...");
    Ok(())
}

/// Configure logging taking verbosity into account
fn init_logging(args: &Args) {
    let log_levels = vec!["error", "warning", "info", "debug"];
    let default_level = 2;
    let actual_level = min(default_level + args.verbose, log_levels.len());
    let env = Env::default()
        .filter_or("LOG_LEVEL", log_levels[actual_level]);
    env_logger::init_from_env(env);
}
