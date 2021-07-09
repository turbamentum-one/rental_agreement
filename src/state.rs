use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_pack::{IsInitialized, Sealed},
    pubkey::Pubkey,
};

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct RentalAgreementAccount {
    pub status: u8,
    pub flat_owner_pubkey: Pubkey,
    pub tenant_pubkey: Pubkey,
    pub deposit: u64,
    pub rent_amount: u64,
    pub duration: u64,
    pub duration_unit: u8,
    pub remaining_payments: u64,
}

impl Sealed for RentalAgreementAccount {}

impl IsInitialized for RentalAgreementAccount {
    fn is_initialized(&self) -> bool {
        self.status != AgreementStatus::Uninitialized as u8
    }
}

impl RentalAgreementAccount {
    pub fn is_complete(&self) -> bool {
        self.status == AgreementStatus::Completed as u8
    }

    pub fn is_terminated(&self) -> bool {
        self.status == AgreementStatus::Terminated as u8
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum Duration {
    Months = 0,
}

#[derive(Copy, Clone)]
pub enum AgreementStatus {
    Uninitialized = 0,
    Active,
    Completed,
    Terminated,
}
