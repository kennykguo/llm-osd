// ABOUTME: provides a user-facing cli for sending action plans to the local executor daemon.
// ABOUTME: prints deterministic json responses returned by the daemon.

use clap::{Parser, Subcommand};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use llmsh::{apply_overrides, parse_and_validate_for_send, validate_verdict};

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
        request_id: Option<String>,

        #[arg(long)]
        session_id: Option<String>,

        #[arg(long)]
        file: Option<String>,

        #[arg(long)]
        json: Option<String>,
    },
    Ping {
        #[arg(long, default_value = "/tmp/llm-osd.sock")]
        socket_path: String,

        #[arg(long, default_value = "req-ping-cli-1")]
        request_id: String,

        #[arg(long)]
        session_id: Option<String>,
    },
    Validate {
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
            request_id,
            session_id,
            file,
            json,
        } => {
            let input = read_input(file.as_deref(), json.as_deref()).await?;
            let plan = parse_and_validate_for_send(&input)?;
            let plan = apply_overrides(plan, request_id.as_deref(), session_id.as_deref())?;
            let canonical = serde_json::to_string(&plan)?;
            let response = send(&socket_path, &canonical).await?;
            print!("{response}");
        }
        Command::Ping {
            socket_path,
            request_id,
            session_id,
        } => {
            let input = format!(
                r#"{{
  "request_id":"{}",
  "version":"0.1",
  "mode":"execute",
  "actions":[{{"type":"ping"}}]
}}"#,
                request_id
            );
            let plan = parse_and_validate_for_send(&input)?;
            let plan = apply_overrides(plan, Some(&request_id), session_id.as_deref())?;
            let canonical = serde_json::to_string(&plan)?;
            let response = send(&socket_path, &canonical).await?;
            print!("{response}");
        }
        Command::Validate { file, json } => {
            let input = read_input(file.as_deref(), json.as_deref()).await?;
            let verdict = validate_verdict(&input);
            print!("{}", serde_json::to_string_pretty(&verdict)?);
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

