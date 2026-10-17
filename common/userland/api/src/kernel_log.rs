use crate::{checked, syscall, Error, Result};

pub fn read(output: &mut [u8]) -> Result<usize> {
    if output.len() > crate::abi::MAX_READ_BUFFER {
        return Err(Error::INVALID_ARGUMENT);
    }
    let length = checked(syscall::invoke(
        crate::abi::SYS_DMESG,
        [output.as_mut_ptr() as u64, output.len() as u64, 0, 0, 0, 0],
    ))?;
    usize::try_from(length).map_err(|_| Error::INVALID_ARGUMENT)
}
