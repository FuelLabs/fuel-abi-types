use thiserror::Error;

/// Revert with this value for a failing call to `std::revert::require`.
pub const FAILED_REQUIRE_SIGNAL: u64 = 0xffff_ffff_ffff_0000;

/// Revert with this value for a failing call to `std::token::transfer_to_address`.
pub const FAILED_TRANSFER_TO_ADDRESS_SIGNAL: u64 = 0xffff_ffff_ffff_0001;

/// Revert with this value for a failing call to `std::message::send_message`.
pub const FAILED_SEND_MESSAGE_SIGNAL: u64 = 0xffff_ffff_ffff_0002;

/// Revert with this value for a failing call to `std::assert::assert_eq`.
pub const FAILED_ASSERT_EQ_SIGNAL: u64 = 0xffff_ffff_ffff_0003;

/// Revert with this value for a failing call to `std::assert::assert`.
pub const FAILED_ASSERT_SIGNAL: u64 = 0xffff_ffff_ffff_0004;

#[derive(Error, Debug)]
pub enum ErrorSignal {
    #[error("Failing call to `std::revert::require`")]
    Require,
    #[error("Failing call to `std::token::transfer_to_address`")]
    TransferToAddress,
    #[error("Failing call to `std::message::send_message`")]
    SendMessage,
    #[error("Failing call to `std::assert::assert_eq`")]
    AssertEq,
    #[error("Failing call to `std::assert::assert`")]
    Assert,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown revert code: {0}")]
    UnknownRevertCode(u64),
    #[error("TODO: Make this make sense")]
    FnSelectorResolving,
}

pub type Result<T> = std::result::Result<T, Error>;

impl ErrorSignal {
    /// Creates a new `ErrorSignal` from provided `revert_code`.
    pub fn try_from_revert_code(revert_code: u64) -> Result<Self> {
        if revert_code == FAILED_REQUIRE_SIGNAL {
            Ok(Self::Require)
        } else if revert_code == FAILED_TRANSFER_TO_ADDRESS_SIGNAL {
            Ok(Self::TransferToAddress)
        } else if revert_code == FAILED_SEND_MESSAGE_SIGNAL {
            Ok(Self::SendMessage)
        } else if revert_code == FAILED_ASSERT_EQ_SIGNAL {
            Ok(Self::AssertEq)
        } else if revert_code == FAILED_ASSERT_SIGNAL {
            Ok(Self::Assert)
        } else {
            Err(Error::UnknownRevertCode(revert_code))
        }
    }

    /// Converts this `ErrorSignal` to corresponding revert code. If the `ErrorSignal` is `Unknown`,
    /// returns `u64::MAX`.
    pub fn to_revert_code(self) -> u64 {
        match self {
            ErrorSignal::Require => FAILED_REQUIRE_SIGNAL,
            ErrorSignal::TransferToAddress => FAILED_TRANSFER_TO_ADDRESS_SIGNAL,
            ErrorSignal::SendMessage => FAILED_SEND_MESSAGE_SIGNAL,
            ErrorSignal::AssertEq => FAILED_ASSERT_EQ_SIGNAL,
            ErrorSignal::Assert => FAILED_ASSERT_SIGNAL,
        }
    }
}
