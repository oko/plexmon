use std::time::Duration;

use clap::Parser;
use isahc::prelude::*;
use isahc::Request;
use plex_api::{library::Library, HttpClientBuilder, Server};
use serde::Deserialize;
use serde::Serialize;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    config: String,
}

#[derive(Deserialize)]
struct ConfigFile {
    config: Config,
}

#[derive(Deserialize)]
struct Config {
    token: String,
    host: String,
    webhook: String,
    username: String,
}

#[derive(Serialize, Deserialize)]
struct Webhook {
    content: String,
    username: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut cfg_fd = OpenOptions::new().read(true).open(args.config).await?;
    let mut cfg_content = String::new();
    cfg_fd.read_to_string(&mut cfg_content).await?;
    let config = toml::from_str::<ConfigFile>(&cfg_content)?.config;

    let client = HttpClientBuilder::default()
        .set_x_plex_token(config.token)
        .build()?;
    let srv = Server::new(config.host, client.clone()).await?;
    let mut output: Vec<String> = vec![];
    for lib in srv.libraries().iter() {
        match lib {
            Library::Movie(movie_lib) => {
                let movies = movie_lib.movies().await?;
                output.push(format!("{}: {} movies", movie_lib.title(), movies.len()));
            }
            Library::TV(tv_lib) => {
                let shows = tv_lib.shows().await?;
                output.push(format!("{}: {} shows", tv_lib.title(), shows.len()));
            }
            Library::Music(music_lib) => {
                let artists = music_lib.artists().await?;
                let mut nalbums = 0;
                let mut ntracks = 0;
                for artist in artists {
                    let albums = artist.albums().await?;
                    nalbums += albums.len();
                    for album in albums {
                        let tracks = album.tracks().await?;
                        ntracks += tracks.len();
                    }
                }
                output.push(format!(
                    "{}: {} albums, {} tracks",
                    music_lib.title(),
                    nalbums,
                    ntracks
                ));
            }
            _ => {
                println!("ignoring library {}", lib.title());
            }
        }
    }
    let w = Webhook {
        content: output.iter().fold(String::new(), |a, b| a + b + "\n"),
        username: config.username,
    };
    let resp = Request::post(config.webhook)
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(3))
        .body(serde_json::to_string(&w)?)?
        .send_async()
        .await?;
    println!("{}", resp.status());
    Ok(())
}
