use crate::{checked, syscall, Error, Result};

pub fn write(bytes: &[u8]) -> Result<usize> {
    if bytes.len() > crate::abi::MAX_READ_BUFFER {
        return Err(Error::INVALID_ARGUMENT);
    }
    let result = checked(syscall::invoke(
        crate::abi::SYS_WRITE_FD,
        [
            crate::abi::FD_STDOUT,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
            0,
            0,
            0,
        ],
    ))?;
    usize::try_from(result).map_err(|_| Error::INVALID_ARGUMENT)
}

pub fn write_all(mut bytes: &[u8]) -> Result<()> {
    while !bytes.is_empty() {
        let written = write(&bytes[..bytes.len().min(crate::abi::MAX_READ_BUFFER)])?;
        if written == 0 {
            return Err(Error::from_errno(5));
        }
        bytes = &bytes[written..];
    }
    Ok(())
}

pub fn read_char() -> Result<char> {
    let mut encoded = [0u8; 4];
    let length = crate::fs::read(crate::fs::stdin(), &mut encoded)?;
    let text = core::str::from_utf8(&encoded[..length]).map_err(|_| Error::INVALID_ARGUMENT)?;
    text.chars().next().ok_or(Error::INVALID_ARGUMENT)
}

pub fn clear() -> Result<()> {
    checked(syscall::invoke(crate::abi::SYS_CLEAR, [0; 6])).map(|_| ())
}

pub fn write_error(bytes: &[u8]) -> Result<usize> {
    if bytes.len() > crate::abi::MAX_READ_BUFFER {
        return Err(Error::INVALID_ARGUMENT);
    }
    let result = checked(syscall::invoke(
        crate::abi::SYS_WRITE_FD,
        [
            crate::abi::FD_STDERR,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
            0,
            0,
            0,
        ],
    ))?;
    usize::try_from(result).map_err(|_| Error::INVALID_ARGUMENT)
}
