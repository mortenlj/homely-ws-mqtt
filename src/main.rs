#[macro_use]
extern crate log;

use std::cmp::min;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

use anyhow::{anyhow, Result};
use async_ctrlc::CtrlC;
use async_std::prelude::FutureExt;
use clap::Parser;
use env_logger::Env;
use reqwest::{Client, Url};
use rust_socketio::{Payload, SocketBuilder};
use serde::Deserialize;
use serde_json::Value;
use tide::Request;
use tokio::task;

const AUTH_URL: &'static str = "https://sdk.iotiliti.cloud/homely/oauth/token";
const LOCATIONS_URL: &'static str = "https://sdk.iotiliti.cloud/homely/locations";
const HOME_URL: &'static str = "https://sdk.iotiliti.cloud/homely/home";
const SOCKET_URL: &'static str = "https://sdk.iotiliti.cloud/";

#[derive(Deserialize, Debug)]
struct Auth {
    access_token: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
struct Location {
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
    let (auth, client) = authenticate(username, password).await?;
    dbg!(&auth);
    let locations = get_locations(&client, &auth).await?;
    dbg!(&locations);

    let state = get_state(&client, &auth, &locations[0]).await?;
    dbg!(&state);

    task::spawn_blocking(move || {
        stream_events(&auth, &locations[0])
    }).await?
}

// XXX: Requires socket.io protocol v3 or v4 (engine.io v3)
fn stream_events(auth: &Auth, location: &Location) -> Result<()> {
    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let pair2 = pair.clone();

    let error_handler = move |err, _ | {
        error!("Error in socket.io: {:#?}", err);
        let (lock, cvar) = &*pair2;
        let mut errored = lock.lock().unwrap();
        *errored = true;
        cvar.notify_one();
    };

    let event_handler = |payload, _| {
        match payload {
            Payload::String(str) => info!("Received: {}", str),
            Payload::Binary(bin_data) => info!("Received bytes: {:#?}", bin_data),
        }
    };

    let mut url = Url::parse(SOCKET_URL)?;
    url.query_pairs_mut()
        .clear()
        .append_pair("locationId", location.location_id.as_str());
    info!("Making socket.io connection to {}", url.as_str());
    SocketBuilder::new(url.as_str())
        .opening_header("Authorization", format!("Bearer {}", auth.access_token))
        .on("event", event_handler)
        .on("error", error_handler)
        .connect()?;

    let (lock, cvar) = &*pair;
    let mut errored = lock.lock().unwrap();
    while !*errored {
        errored = cvar.wait(errored).unwrap();
    }
    Err(anyhow!("Error streaming events!"))
}

async fn get_state(client: &Client, auth: &Auth, location: &Location) -> Result<Value> {
    let state = client.get(format!("{}/{}", HOME_URL, location.location_id))
        .bearer_auth(&auth.access_token)
        .send()
        .await?
        .json::<Value>()
        .await?;
    Ok(state)
}

async fn get_locations(client: &Client, auth: &Auth) -> Result<Vec<Location>> {
    let locations = client.get(LOCATIONS_URL)
        .bearer_auth(&auth.access_token)
        .send()
        .await?
        .json::<Vec<Location>>()
        .await?;
    Ok(locations)
}

/// Authenticate with the REST API and return token
async fn authenticate(username: String, password: String) -> Result<(Auth, Client)> {
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
    Ok((auth, client))
}

/// ^C handler will stop the server
async fn setup_ctrlc_handler() -> Result<()> {
    CtrlC::new().expect("Cannot use CTRL-C handler").await;
    println!("termination signal received, stopping server...");
    Ok(())
}

/// Configure logging taking verbosity into account
fn init_logging(args: &Args) {
    let log_levels = vec!["error", "warning", "info", "debug", "trace"];
    let default_level = 2;
    let actual_level = min(default_level + args.verbose, log_levels.len());
    let env = Env::default()
        .filter_or("LOG_LEVEL", log_levels[actual_level]);
    env_logger::init_from_env(env);
}
