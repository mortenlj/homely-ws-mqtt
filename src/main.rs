#[macro_use]
extern crate log;

use std::borrow::Borrow;
use std::cmp::min;
use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_ctrlc::CtrlC;
use async_std::prelude::FutureExt;
use async_std::task::sleep;
use clap::Parser;
use env_logger::Env;
use serde::Deserialize;
use serde_json::Value;
use tide::Request;

const AUTH_URL: &'static str = "https://sdk.iotiliti.cloud/homely/oauth/token";
const LOCATIONS_URL: &'static str = "https://sdk.iotiliti.cloud/homely/locations";
const HOME_URL: &'static str = "https://sdk.iotiliti.cloud/homely/home";

#[derive(Deserialize, Debug)]
struct Auth {
    access_token: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
struct Location {
    name: String,
    location_id: String
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

    app.race(setup_ctrlc_handler())
        .race(consume_events(args.homely_username, args.homely_password))
        .await?;

    Ok(())
}

async fn probe(mut _req: Request<()>) -> tide::Result {
    debug!("I'm being probed!");
    Ok(format!("I'm alive!").into())
}


async fn consume_events(username: String, password: String) -> Result<()> {
    let auth = authenticate(username, password).await?;
    dbg!(auth.borrow());
    let locations = get_locations(auth.borrow()).await?;
    dbg!(<Vec<Location> as Borrow<[Location]>>::borrow(&locations));

    let state = get_state(auth.borrow(), &locations[0]).await?;
    dbg!(state);

    sleep(Duration::from_secs(20)).await;

    Ok(())
}

async fn get_state(auth: &Auth, location: &Location) -> Result<Value> {
    let client = reqwest::Client::new();
    let state = client.get(format!("{}/{}", HOME_URL, location.location_id))
        .bearer_auth(&auth.access_token)
        .send()
        .await?
        .json::<Value>()
        .await?;
    Ok(state)
}

async fn get_locations(auth: &Auth) -> Result<Vec<Location>> {
    let client = reqwest::Client::new();
    let locations = client.get(LOCATIONS_URL)
        .bearer_auth(&auth.access_token)
        .send()
        .await?
        .json::<Vec<Location>>()
        .await?;
    Ok(locations)
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
