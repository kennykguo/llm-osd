// ABOUTME: runs the privileged daemon that validates and executes structured action plans.
// ABOUTME: exposes a local unix socket and writes an audit log for each request.

mod actions;
mod audit;
mod policy;
mod server;

use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "/tmp/llm-osd.sock")]
    socket_path: String,

    #[arg(long, default_value = "./llm-osd-audit.jsonl")]
    audit_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    server::run(&args.socket_path, &args.audit_path).await
}
