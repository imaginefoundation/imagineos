use crate::{checked, syscall, Result};

pub fn request() -> Result<()> {
    checked(syscall::invoke(crate::abi::SYS_SHUTDOWN_REQUEST, [0; 6])).map(|_| ())
}

pub fn take_request() -> Result<bool> {
    checked(syscall::invoke(crate::abi::SYS_SHUTDOWN_POLL, [0; 6])).map(|value| value != 0)
}

pub fn power_off() -> Result<()> {
    checked(syscall::invoke(crate::abi::SYS_POWER_OFF, [0; 6])).map(|_| ())
}
