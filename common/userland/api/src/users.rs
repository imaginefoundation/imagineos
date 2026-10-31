use crate::{checked, syscall, Error, Result};

pub fn identity() -> Result<crate::abi::UserIdentity> {
    let mut identity = crate::abi::UserIdentity::default();
    checked(syscall::invoke(
        crate::abi::SYS_GETIDENTITY,
        [
            (&mut identity as *mut crate::abi::UserIdentity) as u64,
            0,
            0,
            0,
            0,
            0,
        ],
    ))?;
    Ok(identity)
}

pub fn authenticate(username: &str, password: &[u8], target_uid: u32) -> Result<()> {
    let username = username.as_bytes();
    if username.is_empty()
        || username.len() > crate::abi::ACCOUNT_NAME_SIZE
        || password.len() > crate::abi::ACCOUNT_PASSWORD_SIZE
        || username.contains(&0)
        || password.contains(&0)
    {
        return Err(Error::INVALID_ARGUMENT);
    }
    checked(syscall::invoke(
        crate::abi::SYS_AUTHENTICATE,
        [
            username.as_ptr() as u64,
            username.len() as u64,
            password.as_ptr() as u64,
            password.len() as u64,
            target_uid as u64,
            0,
        ],
    ))
    .map(|_| ())
}
