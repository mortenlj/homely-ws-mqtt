#[macro_use]
extern crate log;

use std::cmp::min;
use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_ctrlc::CtrlC;
use async_std::prelude::FutureExt;
use clap::Parser;
use env_logger::Env;
use serde::Deserialize;
use tide::Request;

const AUTH_URL: &'static str = "https://sdk.iotiliti.cloud/homely/oauth/token";

#[derive(Deserialize)]
struct Auth {
    access_token: String,
}

/// Collect events from Homely API (REST and WebSockets) and send to MQTT
#[derive(Parser, Debug)]
#[clap(author = "Morten Lied Johansen", version, about, long_about = None)]
struct Args {
    /// Listen address
    #[clap(short, long, default_value = "localhost:8080")]
    listen_address: String,

    /// Control verbosity of logs. Can be repeated
    #[clap(short, long, parse(from_occurrences))]
    verbose: usize,

    /// Homely username
    #[clap(long)]
    homely_username: String,

    /// Homely password
    #[clap(long)]
    homely_password: String,
}


#[tokio::main]
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

    let token = authenticate(args.homely_username, args.homely_password).await?;
    debug!("{}", token.access_token);
    app.race(setup_ctrlc_handler()).await?;

    Ok(())
}

async fn probe(mut _req: Request<()>) -> tide::Result {
    debug!("I'm being probed!");
    Ok(format!("I'm alive!").into())
}


/// Authenticate with the REST API and return token
async fn authenticate(username: String, password: String) -> Result<Auth> {
    let mut map = HashMap::new();
    map.insert("username", username);
    map.insert("password", password);

    let client = reqwest::Client::new();
    let auth = client.post(AUTH_URL)
        .json(&map)
        .send()
        .await?
        .json::<Auth>()
        .await?;
    Ok(auth)
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
