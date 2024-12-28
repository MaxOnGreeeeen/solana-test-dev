use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    system_instruction,
};
use solana_sdk::{program::invoke, program_error::ProgramError, rent::Rent, sysvar::Sysvar};

fn process_create_deposit(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    msg!(
        "Missing required signature for user account. {} ",
        program_id
    );

    let deposit_account = next_account_info(accounts_iter)?;
    let user_account = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;

    if !user_account.is_signer {
        msg!("Missing required signature for user account.");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let account_space = 0;
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(account_space);

    msg!(
        "Creating deposit account with {} lamports",
        required_lamports
    );

    invoke(
        &system_instruction::create_account(
            user_account.key,
            deposit_account.key,
            required_lamports,
            account_space as u64,
            program_id,
        ),
        &[
            user_account.clone(),
            deposit_account.clone(),
            system_program.clone(),
        ],
    )?;

    msg!("Deposit account created successfully.");
    Ok(())
}

fn process_balance(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let deposit_account = next_account_info(accounts_iter)?;

    if deposit_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    msg!(
        "Deposit account {} has balance: {} lamports",
        deposit_account.key,
        **deposit_account.lamports.borrow()
    );

    Ok(())
}

fn process_deposit(program_id: &Pubkey, accounts: &[AccountInfo], lamports: u64) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let deposit_account = next_account_info(accounts_iter)?;
    let user_account = next_account_info(accounts_iter)?;

    if !user_account.is_signer {
        msg!("Missing required signature for user account.");
        return Err(ProgramError::MissingRequiredSignature);
    }

    if deposit_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    msg!(
        "on on Lamports {} user lamprots {}",
        lamports,
        user_account.lamports.borrow()
    );

    if **user_account.lamports.borrow() < lamports {
        msg!("Insufficient funds in user account.");
        return Err(ProgramError::InsufficientFunds);
    }

    **user_account.try_borrow_mut_lamports()? -= lamports;
    **deposit_account.try_borrow_mut_lamports()? += lamports;

    msg!(
        "Deposited {} lamports into {}",
        lamports,
        deposit_account.key
    );
    Ok(())
}

fn process_withdraw(program_id: &Pubkey, accounts: &[AccountInfo], lamports: u64) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let deposit_account = next_account_info(accounts_iter)?;
    let user_account = next_account_info(accounts_iter)?;

    if !user_account.is_signer {
        msg!("Missing required signature for user account.");
        return Err(ProgramError::MissingRequiredSignature);
    }

    if deposit_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    msg!(
        "Lamports {} user lamprots {}",
        lamports,
        user_account.lamports.borrow()
    );

    if **deposit_account.lamports.borrow() < lamports {
        msg!("Insufficient funds in deposit account.");
        return Err(ProgramError::InsufficientFunds);
    }

    msg!(
        "Withdraw Lamports {} user lamprots {}",
        lamports,
        user_account.lamports.borrow()
    );

    **deposit_account.try_borrow_mut_lamports()? -= lamports;
    **user_account.try_borrow_mut_lamports()? += lamports;

    msg!(
        "Withdrew {} lamports from {} to {}",
        lamports,
        deposit_account.key,
        user_account.key
    );
    Ok(())
}

entrypoint!(process_instruction);

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum DepositInstruction {
    ProcessCreateDeposit,
    ProcessDepositTranfer { amount: u64 },
    ProcessWithdraw { amount: u64 },
    ProcessBalance,
}
impl DepositInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&variant, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        match variant {
            0 => Ok(Self::ProcessCreateDeposit),
            1 => {
                let lamports = u64::from_le_bytes(
                    rest.try_into()
                        .map_err(|_| ProgramError::InvalidInstructionData)?,
                );
                Ok(Self::ProcessWithdraw { amount: lamports })
            }
            2 => Ok(Self::ProcessBalance),
            3 => {
                let lamports = u64::from_le_bytes(
                    rest.try_into()
                        .map_err(|_| ProgramError::InvalidInstructionData)?,
                );
                Ok(Self::ProcessDepositTranfer { amount: lamports })
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction_type = match DepositInstruction::unpack(&instruction_data) {
        Ok(value) => value,
        Err(_) => Err(solana_program::program_error::ProgramError::InvalidInstructionData)?,
    };

    match instruction_type {
        DepositInstruction::ProcessCreateDeposit => process_create_deposit(program_id, accounts),
        DepositInstruction::ProcessWithdraw { amount } => {
            process_withdraw(program_id, accounts, amount)
        }
        DepositInstruction::ProcessDepositTranfer { amount } => {
            process_deposit(program_id, accounts, amount)
        }
        DepositInstruction::ProcessBalance => process_balance(program_id, accounts),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use solana_program::hash::Hash;
    use solana_program_test::*;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        signature::{Keypair, Signer},
        system_program,
        transaction::Transaction,
        transport::TransportError,
    };

    async fn fund_account(
        banks_client: &mut BanksClient,
        payer: &Keypair,
        recipient: &Pubkey,
        amount: u64,
        recent_blockhash: &Hash,
    ) -> Result<(), BanksClientError> {
        let instruction = system_instruction::transfer(&payer.pubkey(), recipient, amount);
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[payer],
            *recent_blockhash,
        );

        banks_client.process_transaction(transaction).await
    }

    #[tokio::test]
    async fn test_create_deposit_account() -> Result<(), TransportError> {
        let program_id = Pubkey::new_unique();
        let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
            "deposit_program",
            program_id,
            processor!(process_instruction),
        )
        .start()
        .await;

        let deposit_account = Keypair::new();

        let instruction = Instruction::new_with_borsh(
            program_id,
            &DepositInstruction::ProcessCreateDeposit,
            vec![
                AccountMeta::new(deposit_account.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
        );

        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
        transaction.sign(&[&payer, &deposit_account], recent_blockhash);

        banks_client.process_transaction(transaction).await?;

        let deposit_account_data = banks_client
            .get_account(deposit_account.pubkey())
            .await?
            .expect("Deposit account should exist");

        assert_eq!(deposit_account_data.owner, program_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_deposit_and_balance() -> Result<(), TransportError> {
        let program_id = Pubkey::new_unique();
        let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
            "deposit_program",
            program_id,
            processor!(process_instruction),
        )
        .start()
        .await;

        let deposit_account = Keypair::new();
        let deposit_amount = 100_000;

        let create_instruction = Instruction::new_with_borsh(
            program_id,
            &DepositInstruction::ProcessCreateDeposit,
            vec![
                AccountMeta::new(deposit_account.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
        );

        let mut create_transaction =
            Transaction::new_with_payer(&[create_instruction], Some(&payer.pubkey()));
        create_transaction.sign(&[&deposit_account, &payer], recent_blockhash);
        banks_client.process_transaction(create_transaction).await?;

        let deposit_instruction = Instruction::new_with_borsh(
            program_id,
            &DepositInstruction::ProcessDepositTranfer {
                amount: deposit_amount,
            },
            vec![
                AccountMeta::new(deposit_account.pubkey(), false),
                AccountMeta::new(payer.pubkey(), true),
            ],
        );
        let mut deposit_transaction =
            Transaction::new_with_payer(&[deposit_instruction], Some(&payer.pubkey()));
        deposit_transaction.sign(&[&deposit_account, &payer], recent_blockhash);
        banks_client
            .process_transaction(deposit_transaction)
            .await?;

        let deposit_account_data = banks_client
            .get_account(deposit_account.pubkey())
            .await?
            .expect("Deposit account should exist");

        assert_eq!(deposit_account_data.lamports, 890880);
        Ok(())
    }

    #[tokio::test]
    async fn test_withdraw() -> Result<(), TransportError> {
        let program_id = Pubkey::new_unique();
        let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
            "deposit_program",
            program_id,
            processor!(process_instruction),
        )
        .start()
        .await;

        let deposit_account = Keypair::new();
        let deposit_amount = 1_000_000;
        let withdraw_amount = 500_000;

        let create_instruction = Instruction::new_with_borsh(
            program_id,
            &DepositInstruction::ProcessCreateDeposit,
            vec![
                AccountMeta::new(deposit_account.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
        );

        let mut create_transaction =
            Transaction::new_with_payer(&[create_instruction], Some(&payer.pubkey()));
        create_transaction.sign(&[&payer, &deposit_account], recent_blockhash);
        banks_client.process_transaction(create_transaction).await?;

        let payer_account = banks_client
            .get_account(payer.pubkey())
            .await
            .expect("dsfdsf");
        println!("Payer balance {} ", payer_account.unwrap().lamports);

        let deposit_instruction = Instruction::new_with_borsh(
            program_id,
            &DepositInstruction::ProcessDepositTranfer {
                amount: deposit_amount,
            },
            vec![
                AccountMeta::new(deposit_account.pubkey(), false),
                AccountMeta::new(payer.pubkey(), true),
            ],
        );

        let mut deposit_transaction =
            Transaction::new_with_payer(&[deposit_instruction], Some(&payer.pubkey()));
        deposit_transaction.sign(&[&deposit_account, &payer], recent_blockhash);
        banks_client
            .process_transaction(deposit_transaction)
            .await?;

        let withdraw_instruction = Instruction::new_with_borsh(
            program_id,
            &DepositInstruction::ProcessWithdraw {
                amount: withdraw_amount,
            },
            vec![
                AccountMeta::new(deposit_account.pubkey(), false),
                AccountMeta::new(payer.pubkey(), true),
            ],
        );

        let mut withdraw_transaction =
            Transaction::new_with_payer(&[withdraw_instruction], Some(&payer.pubkey()));
        withdraw_transaction.sign(&[&payer], recent_blockhash);
        banks_client
            .process_transaction(withdraw_transaction)
            .await?;

        let deposit_account_data = banks_client
            .get_account(deposit_account.pubkey())
            .await?
            .expect("Deposit account should exist");

        assert_eq!(
            deposit_account_data.lamports,
            deposit_amount - withdraw_amount
        );

        Ok(())
    }
}
