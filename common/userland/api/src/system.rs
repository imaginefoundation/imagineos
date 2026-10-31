//! Informações do sistema em uma única chamada.
//!
//! `SYS_SYSTEM_INFO` existe para que `systemfetch` e `top` não precisem montar o
//! retrato do sistema a partir de várias syscalls: uptime, RAM e contagem de
//! processos vêm do mesmo instante, então os números são coerentes entre si.

use crate::abi::{SYS_SYSTEM_INFO, SystemInfo};
use crate::{Result, checked, syscall};

/// Preenche um `SystemInfo` com o estado atual do sistema.
pub fn info() -> Result<SystemInfo> {
    let mut info = SystemInfo::default();
    checked(syscall::invoke(
        SYS_SYSTEM_INFO,
        [(&mut info as *mut SystemInfo) as u64, 0, 0, 0, 0, 0],
    ))?;
    Ok(info)
}

/// Uptime em microssegundos, do relógio de TSC do kernel.
pub fn uptime_microseconds() -> Result<u64> {
    Ok(info()?.uptime_microseconds)
}

/// RAM utilizável total e livre, em bytes.
pub fn memory() -> Result<(u64, u64)> {
    let info = info()?;
    Ok((info.memory_total, info.memory_free))
}
