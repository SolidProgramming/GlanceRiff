mod collector;
mod fmt;
mod history;
mod model;
mod tui;
mod web;

use std::{net::SocketAddr, time::Duration};

use clap::Parser;

/// A glances-style system monitor: terminal UI or web dashboard.
#[derive(Parser)]
#[command(version, after_help = ENV_HELP)]
struct Args {
    /// Serve a web dashboard and JSON API instead of the terminal UI.
    #[arg(short, long)]
    web: bool,

    /// Address for the web server. Use 0.0.0.0:61208 to expose it on the network.
    #[arg(short, long, default_value = "127.0.0.1:61208")]
    bind: SocketAddr,

    /// Refresh interval in seconds.
    #[arg(short = 't', long, default_value_t = 2.0)]
    interval: f64,

    /// Show process CPU% relative to the whole machine (max 100%) instead of
    /// per core (up to 100% × cores). Can be toggled at runtime.
    #[arg(short, long)]
    normalize_cpu: bool,
}

// Credentials are environment-only: command-line arguments are visible to every
// user on the host (`ps`), the environment only to the process owner and root.
const ENV_HELP: &str = "\
Web server authentication (environment variables, empty means unset):
  GLANCERIFF_USERNAME, GLANCERIFF_PASSWORD  Require HTTP Basic auth (set both)
  GLANCERIFF_TOKEN                          Accept `Authorization: Bearer <TOKEN>`";

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let interval = Duration::from_secs_f64(args.interval.max(0.5));
    if args.web {
        let auth = web::Auth::from_env()?;
        web::run(args.bind, interval, args.normalize_cpu, auth)
    } else {
        tui::run(interval, args.normalize_cpu)
    }
}
