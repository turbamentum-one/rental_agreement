use thiserror::Error;

use solana_program::program_error::ProgramError;

#[derive(Error, Debug, Copy, Clone)]
pub enum RentalAgreementError {
    #[error("Rent Already Paid In Full")]
    AlreadyPaidInFull = 100,

    #[error("Rent Payment Amount Mistmatch")]
    PaymentAmountMismatch,

    #[error("Rent Agreement Terminated")]
    AgreementTerminated,
}

impl From<RentalAgreementError> for ProgramError {
    fn from(e: RentalAgreementError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
