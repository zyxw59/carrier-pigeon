use clap::Parser;
use carrier_pigeon_common::Message;
use tokio::sync::mpsc;
use tracing_subscriber::prelude::*;

#[derive(Debug, Parser)]
struct Args {
    // username: OwnedUserId,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    // let args = Args::parse();
    let log_file = std::sync::Mutex::new(std::fs::File::create("carrier-pigeon.log")?);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(log_file))
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(carrier_pigeon_fake_messages::message_sender(tx.clone()));
    carrier_pigeon_tui::run(rx).await?;
    Ok(())
}

async fn _run(mut messages: mpsc::UnboundedReceiver<Message>) -> color_eyre::Result<()> {
    while let Some(message) = messages.recv().await {
        println!(
            "{} / {} / {} ({})\n{:?}",
            message.key.timestamp,
            message.room.display_name,
            message.sender.display_name,
            message.sender.identifier,
            message.body,
        );
    }
    Ok(())
}
