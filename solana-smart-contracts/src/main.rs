use solana_client::rpc_client::RpcClient;
use solana_program::instruction::Instruction;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::message::Message;
use solana_sdk::signer::Signer;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use std::str::FromStr;

fn main() {
    // Указываем адрес контракта
    let program_id = Pubkey::from_str("YOUR_PROGRAM_ID_HERE").unwrap();

    // Указываем адрес кошелька
    let payer = Keypair::new();
    let client = RpcClient::new("https://api.devnet.solana.com");

    // Создаем инструкцию для депозита (0 - депозита, 1 - вывода)
    let lamports: u64 = 1000000; // Пример: 1 SOL = 1,000,000 лампортов
    let instruction_data = [0, (lamports & 0xFF) as u8]; // Тип операции и сумма

    let accounts = vec![
        AccountMeta::new(payer.try_pubkey().expect("Failed to resolve pubkey"), true),
        AccountMeta::new(program_id, false),
    ];

    // Создаем инструкцию
    let instruction = Instruction::new_with_bytes(program_id, &instruction_data, accounts);
    let message = Message::new(&[instruction], Some(&payer.pubkey()));

    // Создаем и отправляем транзакцию
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let mut transaction = Transaction::new_unsigned(message);
    transaction.recent_blockhash = recent_blockhash;
    transaction.sign(&[&payer], recent_blockhash);

    let result = client.send_and_confirm_transaction(&transaction);
    match result {
        Ok(_) => println!("Transaction successfully sent."),
        Err(err) => eprintln!("Error sending transaction: {}", err),
    }
}
