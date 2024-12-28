use serde::Deserialize;
use solana_client::{client_error::ClientError, rpc_client::RpcClient};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    system_instruction,
    transaction::Transaction,
};
use std::time::Instant;
use std::{fs, str::FromStr, sync::Arc};
use tokio::{sync::mpsc, task::JoinHandle};

static CONFIG_PATH: &str = "config.yaml";
static LAMPORTS: u64 = 2000000;

#[derive(Debug, Deserialize)]
struct Wallet {
    private_key: String,
    public_key: String,
}
struct SenderWallet {
    private_key: Keypair,
    public_key: Pubkey,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct ReceiverWallet {
    public_key: Pubkey,
}

#[derive(Debug, Deserialize)]
struct Config {
    wallets: Vec<Wallet>,
    receivers: Vec<String>,
    rpc_url: String,
}

// Отправка транзакции
async fn send_sol(
    client: &RpcClient,
    sender: &Keypair,
    receiver: &Pubkey,
    amount: u64,
) -> Result<Signature, Box<dyn std::error::Error + Send + Sync>> {
    let instruction = system_instruction::transfer(&sender.pubkey(), receiver, amount);
    let recent_blockhash = client
        .get_latest_blockhash()
        .expect("Cannot get latest blockhash");

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&sender.pubkey()),
        &[sender],
        recent_blockhash,
    );

    let signature = client.send_and_confirm_transaction(&transaction)?;

    Ok(signature)
}

// Проверка статуса транзакции
async fn check_transaction_status(
    client: &RpcClient,
    signature: &Signature,
) -> Result<(), ClientError> {
    return match client.get_signature_status(signature) {
        Ok(value) => match value {
            Some(value) => match value {
                Ok(_) => Ok(()),
                Err(err) => {
                    println!("Transaction error!");
                    Err(err.into())
                }
            },
            None => return Ok(()),
        },
        Err(err) => {
            println!("Transaction error!");
            Err(err.into())
        }
    };
}

// С каждого кошелька отправляем транзакции всем другим кошелькам
async fn send_transactions<'a>(config: &'a Config, client: Arc<RpcClient>) {
    let mut tasks: Vec<JoinHandle<Result<(), ()>>> = vec![];
    let (senders, receivers) = process_wallets(config);

    for sender_wallet in senders {
        let sender_ref = Arc::new(sender_wallet);
        let client = Arc::clone(&client);

        receivers.iter().for_each(|receiver_wallet| {
            let sender_ref = Arc::clone(&sender_ref);
            let receiver_ref = Arc::new(*receiver_wallet);
            let client = Arc::clone(&client);

            let task = tokio::spawn(async move {
                let start_time = Instant::now();

                match send_sol(
                    &client,
                    &sender_ref.private_key,
                    &receiver_ref.public_key,
                    LAMPORTS,
                )
                .await
                {
                    Ok(signature) => {
                        let duration = start_time.elapsed();

                        println!("Transaction Hash: {:?}, Time: {:?}", signature, duration);

                        match check_transaction_status(&client, &signature).await {
                            Ok(value) => Ok(value),
                            Err(err) => {
                                println!(
                                    "Error sending from wallet {} to wallet {}: {:?}",
                                    &sender_ref.public_key, &receiver_ref.public_key, err
                                );
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => {
                        println!(
                            "Error sending from wallet {}: {:?}",
                            &sender_ref.public_key, e
                        );
                        return Ok(());
                    }
                }
            });

            tasks.push(task);
        })
    }

    for task in tasks {
        let _ = task.await;
    }
}

#[tokio::main]
async fn main() {
    let config_content = fs::read_to_string(CONFIG_PATH).expect("Unable to read config file");
    let config: Config = serde_yaml::from_str(&config_content).expect("Unable to parse config");
    let client = RpcClient::new(config.rpc_url.clone());
    let client_ref = Arc::new(client);

    send_transactions(&config, client_ref).await;
}

fn process_wallets(config: &Config) -> (Vec<SenderWallet>, Vec<ReceiverWallet>) {
    return (
        config
            .wallets
            .iter()
            .map(|sender| {
                let bytes =
                    parse_bytes_from_string(&sender.private_key).expect("Failed to convert bytes");

                let sender_public_key: Pubkey = get_public_key(&sender.public_key);
                let sender_keypair =
                    Keypair::from_bytes(&bytes).expect("Failed to parse private key");

                return SenderWallet {
                    public_key: sender_public_key,
                    private_key: sender_keypair,
                };
            })
            .collect(),
        config
            .receivers
            .iter()
            .map(|public_key| {
                let receiver_public_key: Pubkey = get_public_key(public_key);

                return ReceiverWallet {
                    public_key: receiver_public_key,
                };
            })
            .collect(),
    );
}

#[inline(always)]
fn get_public_key(public_key: &str) -> Pubkey {
    return Pubkey::from_str(&public_key).expect("Failed to parse public key");
}

#[inline(always)]
fn parse_bytes_from_string(input: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim_matches(['[', ']'].as_ref());
    let result: Result<Vec<u8>, _> = trimmed
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<u16>()
                .map_err(|e| format!("Failed to parse number: {}", e))
                .and_then(|num| {
                    if num > 255 {
                        Err(format!("Number {} out of byte range", num))
                    } else {
                        Ok(num as u8)
                    }
                })
        })
        .collect();

    result
}
