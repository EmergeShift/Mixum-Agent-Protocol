//! Minimal Rust agent harness: connect to a MAP node and submit a task.
//!
//! ```text
//! MAP_TOKEN=map1... cargo run -p map-client --example rust_agent_sandbox -- wss://127.0.0.1:7601/ws
//! ```

use map_client::MapClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "wss://127.0.0.1:7601/ws".to_owned());
    let token = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("MAP_TOKEN").ok());
    let Some(token) = token else {
        eprintln!("usage: rust_agent_sandbox <ws-url> <map1.token>  (or set MAP_TOKEN)");
        std::process::exit(2);
    };

    let fingerprint = std::env::var("MAP_NODE_FINGERPRINT").ok();
    let (mut client, hello) = MapClient::connect_with_fingerprint(
        &url,
        &token,
        "map-client-example/0.1",
        fingerprint.as_deref(),
    )
    .await?;
    println!("connected: server={} arch={}", hello.server, hello.arch);

    let info = client.call("sys.info", json!({})).await?;
    println!("sys.info: {info}");

    let task = client
        .call(
            "task.submit",
            json!({ "instruction": "echo hello from map-client", "agent": "shell" }),
        )
        .await?;
    println!("task.submit: {task}");

    client.close().await;
    Ok(())
}
