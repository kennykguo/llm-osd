// ABOUTME: provides a user-facing cli for sending action plans to the local executor daemon.
// ABOUTME: prints deterministic json responses returned by the daemon.

use clap::{Parser, Subcommand};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[derive(Debug, Parser)]
#[command(name = "llmsh")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Send {
        #[arg(long, default_value = "/tmp/llm-osd.sock")]
        socket_path: String,

        #[arg(long)]
        file: Option<String>,

        #[arg(long)]
        json: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Send {
            socket_path,
            file,
            json,
        } => {
            let input = read_input(file.as_deref(), json.as_deref()).await?;
            let response = send(&socket_path, &input).await?;
            print!("{response}");
        }
    }

    Ok(())
}

async fn read_input(file: Option<&str>, json: Option<&str>) -> anyhow::Result<String> {
    if let Some(json) = json {
        return Ok(json.to_string());
    }

    if let Some(file) = file {
        return Ok(tokio::fs::read_to_string(file).await?);
    }

    let mut input = String::new();
    tokio::io::stdin().read_to_string(&mut input).await?;
    Ok(input)
}

async fn send(socket_path: &str, input: &str) -> anyhow::Result<String> {
    let mut stream = UnixStream::connect(socket_path).await?;
    stream.write_all(input.as_bytes()).await?;
    stream.shutdown().await?;

    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}
