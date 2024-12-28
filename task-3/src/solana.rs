use solana_client::{client_error::ClientError, rpc_client::RpcClient};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;

// Отправка транзакции
pub async fn send_sol(
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
pub async fn check_transaction_status(
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

#[inline(always)]
pub fn get_public_key(public_key: &str) -> Pubkey {
    return Pubkey::from_str(&public_key).expect("Failed to parse public key");
}

#[inline(always)]
pub fn parse_bytes_from_string(input: &str) -> Result<Vec<u8>, String> {
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
