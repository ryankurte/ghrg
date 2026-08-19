//! A simple CLI for fetching GitHub release artifacts from (public|private) repositories.

use clap::Parser;
use tracing::{debug, error, info};
use tracing_subscriber::filter::LevelFilter;

use ghrg::ReleaseClient;

#[derive(clap::Parser, Debug)]
pub struct Args {
    /// The GitHub repository to fetch the release artifact from.
    #[clap()]
    pub repo: String,

    /// The GitHub release tag to fetch the artifact from.
    #[clap(long)]
    pub tag: Option<String>,

    /// The name(s) of the artifact(s) to fetch from the release.
    #[clap(long)]
    pub asset: Vec<String>,

    /// The output directory to save the fetched artifact(s).
    #[clap(long, default_value = ".")]
    pub output: String,

    /// The GitHub Personal Access Token (PAT) to use for authentication.
    #[clap(long, env = "GITHUB_TOKEN")]
    pub gh_token: Option<String>,

    /// Log level for the application.
    #[clap(long, default_value = "info")]
    pub log_level: LevelFilter,
}

fn main() {
    // Parse the command-line arguments
    let args = Args::parse();

    // Setup a tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .init();

    if args.gh_token.is_some() {
        debug!("Found GITHUB_TOKEN");
    } else {
        debug!("No GITHUB_TOKEN found, proceeding without authentication");
    }

    // Setup the HTTP client
    let client = ReleaseClient::new(&args.repo, args.gh_token.as_deref());

    info!("Fetching release artifacts from repository: {}", args.repo);

    // Fetch the release information
    let release_info = client.fetch_release_info(args.tag.as_deref()).expect("Failed to fetch release info");

    // Fetch the release assets
    let assets = release_info.assets();
    debug!("Found {} assets in release", assets.len());

    let globs = args.asset.iter().map(|s| glob::Pattern::new(s).expect("Invalid glob pattern")).collect::<Vec<_>>();

    // Filter the assets based on the specified artifact names
    let filtered_assets: Vec<_> = assets.iter().filter(|a| {
        globs.iter().any(|g| g.matches(&a.name))
    }).collect();

    if filtered_assets.is_empty() {
        error!("No matching artifacts found for the specified patterns.");
        return;
    }

    debug!("Matched {} assets for download", filtered_assets.len());

    // Download the filtered assets
    for asset in filtered_assets {
        info!("Downloading: {}", asset.name);
        let output_path = std::path::Path::new(&args.output).join(&asset.name);
        client.download_asset(&asset, &output_path).expect("Failed to download asset");
        debug!("Downloaded complete");
    }

}
