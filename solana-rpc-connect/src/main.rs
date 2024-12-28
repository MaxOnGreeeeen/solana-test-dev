use reqwest::Client;
use serde::Deserialize;
use std::fs;
use tokio::time::{sleep, Duration};

static CONFIG_PATH: &str = "config.yaml";

#[derive(Deserialize)]
struct Config {
    wallets: Vec<String>,
    rcp_url: String,
}

async fn health_check(rpc_url: &str, client: &Client) -> Result<bool, String> {
    let response = client.get(rpc_url).send().await;
    match response {
        Ok(_) => Ok(true),
        Err(err) => Err(err.to_string()),
    }
}

async fn get_balance(
    wallet: String,
    rpc_url: &str,
    client: &Client,
) -> (String, Result<u64, String>) {
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBalance",
        "params": [wallet]
    });

    let response = client.post(rpc_url).json(&request_body).send().await;

    match response {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => {
                if let Some(balance) = json
                    .get("result")
                    .and_then(|r| r.get("value"))
                    .and_then(|v| v.as_u64())
                {
                    (wallet, Ok(balance))
                } else {
                    (wallet, Err("Failed to parse balance".into()))
                }
            }
            Err(_) => (wallet, Err("Failed to parse JSON response".into())),
        },
        Err(err) => (wallet, Err(err.to_string())),
    }
}

async fn get_balances(
    http_client: &Client,
    wallets: Vec<String>,
    rpc_url: &str,
) -> Vec<(String, Result<u64, String>)> {
    let mut tasks: Vec<tokio::task::JoinHandle<(String, Result<u64, String>)>> = Vec::new();

    for wallet_address in wallets {
        let http_client = http_client.clone();
        let rpc_url = rpc_url.to_string();

        let task =
            tokio::spawn(async move { get_balance(wallet_address, &rpc_url, &http_client).await });
        tasks.push(task);
    }

    let mut results = Vec::new();
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    results
}

#[tokio::main]
async fn main() {
    let config_content = fs::read_to_string(CONFIG_PATH).expect("Failed to read config file");
    let config: Config =
        serde_yaml::from_str(&config_content).expect("Failed to parse config file");

    if config.wallets.is_empty() {
        println!("No wallets found in config file.");
        return;
    }

    let http_client = Client::new();
    let rpc_url = config.rcp_url;
    let balances = get_balances(&http_client, config.wallets, &rpc_url).await;

    loop {
        let rpc_url = rpc_url.clone();
        let http_client = http_client.clone();

        println!("Health check...");

        let healt_check_req = tokio::spawn(async move {
            let rpc_url = rpc_url.clone();
            health_check(&rpc_url, &http_client).await
        });

        if let Ok(_) = healt_check_req.await {
            println!("Health check completed...");
            break;
        } else {
            println!("Server is not responding, retry in 3 seconds...");
            sleep(Duration::from_secs(3)).await;
        }
    }

    for (wallet, balance) in balances {
        match balance {
            Ok(amount) => println!("Wallet: {}, Balance: {}", wallet, amount),
            Err(err) => println!("Wallet: {}, Error: {}", wallet, err),
        }
    }
}
