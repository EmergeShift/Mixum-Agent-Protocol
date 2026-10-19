//! Minimal Rust agent harness: connect to a MAP node and submit a task.
//!
//! ```text
//! MAP_TOKEN=map1... cargo run -p map-client --example rust_agent_sandbox -- ws://127.0.0.1:7601
//! ```

use map_client::MapClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ws://127.0.0.1:7601".to_owned());
    let token = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("MAP_TOKEN").ok());
    let Some(token) = token else {
        eprintln!("usage: rust_agent_sandbox <ws-url> <map1.token>  (or set MAP_TOKEN)");
        std::process::exit(2);
    };

    let (mut client, hello) = MapClient::connect(&url, &token, "map-client-example/0.1").await?;
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
