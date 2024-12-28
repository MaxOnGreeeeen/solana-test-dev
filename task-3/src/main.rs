use serde::Deserialize;
use solana::{check_transaction_status, get_public_key, parse_bytes_from_string, send_sol};
use solana_sdk::signature::Keypair;
use std::collections::HashMap;
use std::{fs, sync::Arc};
use yellowstone_grpc_client::GeyserGrpcClient;
mod solana;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio::time::Instant;
use yellowstone_grpc_proto::geyser::{SubscribeRequest, SubscribeRequestFilterBlocks};

use futures_util::StreamExt;
use solana_client::rpc_client::RpcClient;

static CONFIG_PATH: &str = "config.yaml";

#[derive(Debug, Deserialize)]
struct Config {
    sender_private_key: String,
    sender_public_key: String,
    recipient_wallet: String,
    solana_rpc_url: String,
    gayser_rpc_url: String,
    geyser_x_token: String,
    amount: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_content = fs::read_to_string(CONFIG_PATH).expect("Unable to read config file");
    let config: Config = serde_yaml::from_str(&config_content).expect("Unable to parse config");

    let builder = match GeyserGrpcClient::build_from_shared(config.gayser_rpc_url) {
        Ok(value) => value,
        Err(err) => {
            println!("Error occured {}", err);
            panic!("Failed to build GeyserGrpcClient");
        }
    };

    let mut client = builder
        .x_token(Some(config.geyser_x_token))
        .expect("Failed to add token")
        .connect()
        .await
        .unwrap();
    let mut blocks: HashMap<String, SubscribeRequestFilterBlocks> = HashMap::new();
    blocks.insert(
        "blocks".to_string(),
        SubscribeRequestFilterBlocks {
            account_include: vec![config.sender_public_key],
            ..SubscribeRequestFilterBlocks::default()
        },
    );

    let mut request_filter: SubscribeRequest = SubscribeRequest::default();
    request_filter.blocks = blocks;
    let request = Some(request_filter);
    let (_, mut stream) = client.subscribe_with_request(request).await.map_err(|e| {
        eprintln!("Failed to subscribe: {:?}", e);
        e
    })?;

    let solana_rpc_client = RpcClient::new(config.solana_rpc_url);
    let (tx, mut rx) = mpsc::channel::<String>(8);
    let tx_ref = Arc::new(tx);

    let _task: tokio::task::JoinHandle<Result<(), ()>> = tokio::spawn(async move {
        let bytes =
            parse_bytes_from_string(&config.sender_private_key).expect("Failed to convert bytes");
        let sender_private_key = Keypair::from_bytes(&bytes).expect("Failed to parse private key");
        let receiver_public_key: Pubkey = get_public_key(&config.recipient_wallet);

        Ok(loop {
            match rx.recv().await {
                Some(_) => {
                    let start_time = Instant::now();

                    match send_sol(
                        &solana_rpc_client,
                        &sender_private_key,
                        &receiver_public_key,
                        config.amount,
                    )
                    .await
                    {
                        Ok(signature) => {
                            let duration = start_time.elapsed();

                            println!("Transaction Hash: {:?}, Time: {:?}", signature, duration);

                            match check_transaction_status(&solana_rpc_client, &signature).await {
                                Ok(_) => (),
                                Err(err) => {
                                    println!("Error sending transaction {}", err);
                                    return Ok(());
                                }
                            }
                        }
                        Err(e) => {
                            println!("Error sending from wallet transaction",);
                            return Ok(());
                        }
                    }
                }
                None => {
                    println!("Channel closed, no more messages to receive.");
                    break;
                }
            }
        })
    });

    while let Some(update) = stream.next().await {
        match update {
            Ok(data) => {
                println!("Update from subscribtion, {:?}", data.update_oneof);
                let tx_ref = Arc::clone(&tx_ref);

                tokio::spawn(async move {
                    tx_ref.send("".to_string()).await;
                });
            }
            Err(e) => {
                eprintln!("Error receiving update: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
