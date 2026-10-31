#![no_std]

pub use imagineos_abi as abi;

pub mod args;
pub mod console;
pub mod display;
pub mod fmt;
pub mod font;
pub mod fs;
pub mod kernel_log;
pub mod legacy;
pub mod process;
pub mod shutdown;
pub mod signals;
pub mod surface;
pub mod syscall;
pub mod system;
pub mod users;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    code: i32,
}

impl Error {
    pub const INVALID_ARGUMENT: Self = Self { code: 22 };
    pub const TOO_BIG: Self = Self { code: 7 };
    pub const NO_SPACE: Self = Self { code: 28 };
    /// O arquivo existe mas não é um executável DEF válido.
    pub const NO_EXECUTABLE: Self = Self { code: 8 };
    pub const NOT_FOUND: Self = Self { code: 2 };
    pub const PERMISSION_DENIED: Self = Self { code: 13 };
    pub const OUT_OF_MEMORY: Self = Self { code: 12 };
    /// Nenhum slot de processo livre (ou recurso esgotado).
    pub const UNAVAILABLE: Self = Self { code: 11 };

    pub const fn code(self) -> i32 {
        self.code
    }

    pub const fn from_errno(code: i32) -> Self {
        Self {
            code: code.saturating_abs(),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn checked(value: u64) -> Result<u64> {
    let signed = value as i64;
    if signed < 0 {
        Err(Error::from_errno(signed.saturating_abs() as i32))
    } else {
        Ok(value)
    }
}

pub(crate) fn path_bytes(path: &str, max_len: usize) -> Result<&[u8]> {
    let bytes = path.as_bytes();
    if bytes.is_empty() || bytes.len() > max_len || bytes.contains(&0) {
        return Err(Error::INVALID_ARGUMENT);
    }
    Ok(bytes)
}
