use core::arch::asm;

pub(crate) fn invoke(number: u64, arguments: [u64; 6]) -> u64 {
    let [rdi, rsi, rdx, r10, r8, r9] = arguments;
    let result: u64;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") number => result,
            in("rdi") rdi,
            in("rsi") rsi,
            in("rdx") rdx,
            in("r10") r10,
            in("r8") r8,
            in("r9") r9,
        );
    }
    result
}

pub fn abi_version() -> crate::Result<u64> {
    crate::checked(invoke(crate::abi::SYS_ABI_VERSION, [0; 6]))
}
