mod args;
mod net;
mod renderer;
mod window;
mod world;

use std::path::PathBuf;

use clap::Parser;

use net::connection::ConnectArgs;

fn main() {
    env_logger::init();

    let args = args::LaunchArgs::parse();

    let assets_dir: PathBuf = args
        .assets_dir
        .as_deref()
        .unwrap_or("reference/assets")
        .into();

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    let event_rx = if let Some(ref server) = args.server {
        let connect_args = ConnectArgs {
            server: server.clone(),
            username: args.username.clone().unwrap_or_else(|| "Steve".into()),
            uuid: args
                .uuid
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(uuid::Uuid::nil),
            access_token: args.access_token.clone(),
        };

        let (event_tx, event_rx) = crossbeam_channel::bounded(256);

        rt.spawn(async move {
            if let Err(e) = net::connection::connect_to_server(connect_args, event_tx).await {
                log::error!("Network error: {e}");
            }
        });

        Some(event_rx)
    } else {
        None
    };

    if let Err(e) = window::run(event_rx, assets_dir) {
        log::error!("Fatal: {e}");
        std::process::exit(1);
    }

    drop(rt);
}
