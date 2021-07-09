use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    program_pack::IsInitialized,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};

use crate::{
    error::RentalAgreementError,
    instruction::RentalInstruction,
    state::{AgreementStatus, RentalAgreementAccount},
};

static LOG_TAG_NAME: &str = "[rental_agreement]";

pub struct Processor;
impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = RentalInstruction::unpack(instruction_data)?;

        match instruction {
            RentalInstruction::Initialization {
                flat_owner_pubkey,
                tenant_pubkey,
                deposit,
                rent_amount,
                duration,
                duration_unit,
            } => Self::initialize_rent_contract(
                accounts,
                program_id,
                flat_owner_pubkey,
                tenant_pubkey,
                deposit,
                rent_amount,
                duration,
                duration_unit,
            ),

            RentalInstruction::Payment { rent_amount } => {
                Self::pay_rent(accounts, program_id, rent_amount)
            }

            RentalInstruction::TerminationBeforeInitialDate {} => {
                Self::terminate_before_initial_date(accounts, program_id)
            }
        }
    }

    fn initialize_rent_contract(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        payee_pubkey: Pubkey,
        payer_pubkey: Pubkey,
        deposit: u64,
        rent_amount: u64,
        duration: u64,
        duration_unit: u8,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let rent_agreement_account = next_account_info(accounts_iter)?;
        if rent_agreement_account.owner != program_id {
            msg!(
                "{} Rent agreement account not owned by this program",
                LOG_TAG_NAME
            );
            return Err(ProgramError::IncorrectProgramId);
        }

        let solana_rent = &Rent::from_account_info(next_account_info(accounts_iter)?)?;
        if !solana_rent.is_exempt(
            rent_agreement_account.lamports(),
            rent_agreement_account.data_len(),
        ) {
            msg!(
                "{} Rental agreement account not rent exempt on Solana; balance: {}",
                LOG_TAG_NAME,
                rent_agreement_account.lamports()
            );
            return Err(ProgramError::AccountNotRentExempt);
        }

        // Initialize the Rent Agreement Account with the initial data
        // Note: the structure of the data state must match the `space` reserved when account created
        let rent_agreement_data =
            RentalAgreementAccount::try_from_slice(&rent_agreement_account.data.borrow());
        if rent_agreement_data.is_err() {
            msg!(
                "{} Rent agreement account data size incorrect: {}",
                LOG_TAG_NAME,
                rent_agreement_account.try_data_len()?
            );
            return Err(ProgramError::InvalidAccountData);
        }

        let mut rent_data = rent_agreement_data.unwrap();
        if rent_data.is_initialized() {
            msg!("{} Rent agreement already initialized", LOG_TAG_NAME);
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        rent_data.status = AgreementStatus::Active as u8;
        rent_data.flat_owner_pubkey = payee_pubkey;
        rent_data.tenant_pubkey = payer_pubkey;
        rent_data.rent_amount = rent_amount;
        rent_data.deposit = deposit;
        rent_data.duration = duration;
        rent_data.duration_unit = duration_unit;
        rent_data.remaining_payments = duration;
        rent_data.serialize(&mut &mut rent_agreement_account.data.borrow_mut()[..])?;

        msg!(
            "{} Initialized rent agreement account: {:?}",
            LOG_TAG_NAME,
            rent_data
        );

        Ok(())
    }

    fn pay_rent(accounts: &[AccountInfo], program_id: &Pubkey, rent_amount: u64) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let rent_agreement_account = next_account_info(accounts_iter)?;
        if rent_agreement_account.owner != program_id {
            msg!(
                "{}, Rent agreement account is not owned by this program",
                LOG_TAG_NAME
            );
            return Err(ProgramError::IncorrectProgramId);
        }

        let flat_owner_account: &AccountInfo = next_account_info(accounts_iter)?;
        let tenant_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;

        if !tenant_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        if tenant_account.lamports() < rent_amount {
            return Err(ProgramError::InsufficientFunds);
        }

        // Transfer to self - do nothing
        if tenant_account.key == flat_owner_account.key {
            return Ok(());
        }

        // Initialize the Rent Agreement Account with the initial data
        // Note: the structure of the data state must match the `space` the client used to create the account
        let rent_agreement_data =
            RentalAgreementAccount::try_from_slice(&rent_agreement_account.data.borrow());

        if rent_agreement_data.is_err() {
            msg!(
                "{} Rent agreement account data size incorrect: {}",
                LOG_TAG_NAME,
                rent_agreement_account.try_data_len()?
            );
            return Err(ProgramError::InvalidAccountData);
        }

        let mut rent_data = rent_agreement_data.unwrap();
        if !rent_data.is_initialized() {
            msg!("{} Rent agreement account not initialized", LOG_TAG_NAME);
            return Err(ProgramError::UninitializedAccount);
        }

        // Make sure we pay the same account used during the agreement initialization
        if rent_data.flat_owner_pubkey != *flat_owner_account.key {
            msg!(
                "{} Payee must match payee key used during agreement initialization",
                LOG_TAG_NAME
            );
            return Err(ProgramError::InvalidAccountData);
        }

        msg!(
            "{} Transfer {} lamports from payer with balance: {}",
            LOG_TAG_NAME,
            rent_amount,
            tenant_account.lamports()
        );

        if rent_data.is_complete() {
            msg!("{} Rent already paid in full", LOG_TAG_NAME);
            return Err(RentalAgreementError::AlreadyPaidInFull.into());
        }

        if rent_data.is_terminated() {
            msg!("{} Rent agreement already terminated", LOG_TAG_NAME);
            return Err(RentalAgreementError::AgreementTerminated.into());
        }

        if rent_data.rent_amount != rent_amount {
            msg!(
                "{} Rent amount does not match agreement amount: {} vs {}",
                LOG_TAG_NAME,
                rent_data.rent_amount,
                rent_amount
            );
            return Err(RentalAgreementError::PaymentAmountMismatch.into());
        }

        let instruction =
            system_instruction::transfer(&tenant_account.key, &flat_owner_account.key, rent_amount);

        // Invoke the system program to transfer funds
        invoke(
            &instruction,
            &[
                system_program_account.clone(),
                flat_owner_account.clone(),
                tenant_account.clone(),
            ],
        )?;

        msg!(
            "{} Transfer completed. New payer balance: {}",
            LOG_TAG_NAME,
            tenant_account.lamports()
        );

        // Decrement the number of payment
        rent_data.remaining_payments -= 1;
        if rent_data.remaining_payments == 0 {
            rent_data.status = AgreementStatus::Completed as u8;
        }
        rent_data.serialize(&mut &mut rent_agreement_account.data.borrow_mut()[..])?;

        Ok(())
    }

    fn terminate_before_initial_date(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let rent_agreement_account = next_account_info(accounts_iter)?;
        if rent_agreement_account.owner != program_id {
            msg!(
                "{} Rent agreement account is not owned by this program",
                LOG_TAG_NAME
            );
            return Err(ProgramError::IncorrectProgramId);
        }

        let rent_agreement_data =
            RentalAgreementAccount::try_from_slice(&rent_agreement_account.data.borrow());

        if rent_agreement_data.is_err() {
            msg!(
                "{} Rent agreement account data size incorrect: {}",
                LOG_TAG_NAME,
                rent_agreement_account.try_data_len()?
            );
            return Err(ProgramError::InvalidAccountData);
        }

        let mut rent_data = rent_agreement_data.unwrap();
        if !rent_data.is_initialized() {
            msg!("{} Rent agreement account not initialized", LOG_TAG_NAME);
            return Err(ProgramError::UninitializedAccount);
        }

        if rent_data.is_complete() {
            msg!("{} Rent already paid in full", LOG_TAG_NAME);
            return Err(RentalAgreementError::AlreadyPaidInFull.into());
        }

        if rent_data.is_terminated() {
            msg!("{} Rent agreement already terminated", LOG_TAG_NAME);
            return Err(RentalAgreementError::AgreementTerminated.into());
        }

        rent_data.remaining_payments = 0;
        rent_data.status = AgreementStatus::Terminated as u8;
        rent_data.serialize(&mut &mut rent_agreement_account.data.borrow_mut()[..])?;

        Ok(())
    }
}
