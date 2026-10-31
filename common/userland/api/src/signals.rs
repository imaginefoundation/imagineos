use crate::{checked, syscall, Error, Result};

pub const SIGHUP: u64 = crate::abi::SIGNAL_HUP;
pub const SIGINT: u64 = crate::abi::SIGNAL_INT;
pub const SIGKILL: u64 = crate::abi::SIGNAL_KILL;
pub const SIGSEGV: u64 = crate::abi::SIGNAL_SEGV;
pub const SIGTERM: u64 = crate::abi::SIGNAL_TERM;
pub const SIGCONT: u64 = crate::abi::SIGNAL_CONT;
pub const SIGSTOP: u64 = crate::abi::SIGNAL_STOP;

pub type Handler = extern "C" fn(i32);

unsafe extern "C" {
    fn imagineos_signal_restorer();
}

pub fn send(pid: usize, signal: u64) -> Result<()> {
    checked(syscall::invoke(
        crate::abi::SYS_KILL,
        [pid as u64, signal, 0, 0, 0, 0],
    ))
    .map(|_| ())
}

pub fn register(signal: u64, handler: Option<Handler>) -> Result<()> {
    if signal == SIGKILL || signal == SIGSTOP {
        return Err(Error::INVALID_ARGUMENT);
    }
    let handler = handler.map_or(crate::abi::SIGNAL_DEFAULT, |handler| {
        handler as usize as u64
    });
    let restorer = imagineos_signal_restorer as *const () as usize as u64;
    checked(syscall::invoke(
        crate::abi::SYS_SIGACTION,
        [signal, handler, restorer, 0, 0, 0],
    ))
    .map(|_| ())
}

pub fn ignore(signal: u64) -> Result<()> {
    if signal == SIGKILL || signal == SIGSTOP {
        return Err(Error::INVALID_ARGUMENT);
    }
    checked(syscall::invoke(
        crate::abi::SYS_SIGACTION,
        [signal, crate::abi::SIGNAL_IGNORE, 0, 0, 0, 0],
    ))
    .map(|_| ())
}

pub fn restore_context() -> ! {
    let _ = syscall::invoke(crate::abi::SYS_SIGRETURN, [0; 6]);
    loop {
        core::hint::spin_loop();
    }
}

core::arch::global_asm!(
    ".global imagineos_signal_restorer",
    ".type imagineos_signal_restorer,@function",
    "imagineos_signal_restorer:",
    "mov rax, {sigreturn}",
    "int 0x80",
    "ud2",
    ".size imagineos_signal_restorer, .-imagineos_signal_restorer",
    sigreturn = const crate::abi::SYS_SIGRETURN,
);
