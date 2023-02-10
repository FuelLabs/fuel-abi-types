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
    #[error("Unknown error signal")]
    Unknown,
}

impl ErrorSignal {
    /// Creates a new `ErrorSignal` from provided `revert_code`.
    pub fn from_revert_code(revert_code: u64) -> Self {
        if revert_code == FAILED_REQUIRE_SIGNAL {
            Self::Require
        } else if revert_code == FAILED_TRANSFER_TO_ADDRESS_SIGNAL {
            Self::TransferToAddress
        } else if revert_code == FAILED_SEND_MESSAGE_SIGNAL {
            Self::SendMessage
        } else if revert_code == FAILED_ASSERT_EQ_SIGNAL {
            Self::AssertEq
        } else if revert_code == FAILED_ASSERT_SIGNAL {
            Self::Assert
        } else {
            Self::Unknown
        }
    }

    /// Converts this `ErrorSignal` to corresponding revert code. If the `ErroSignal` is `Unknown`,
    /// returns `u64::MAX`.
    pub fn to_revert_code(self) -> u64 {
        match self {
            ErrorSignal::Require => FAILED_REQUIRE_SIGNAL,
            ErrorSignal::TransferToAddress => FAILED_TRANSFER_TO_ADDRESS_SIGNAL,
            ErrorSignal::SendMessage => FAILED_SEND_MESSAGE_SIGNAL,
            ErrorSignal::AssertEq => FAILED_ASSERT_EQ_SIGNAL,
            ErrorSignal::Assert => FAILED_ASSERT_SIGNAL,
            ErrorSignal::Unknown => u64::MAX,
        }
    }
}
