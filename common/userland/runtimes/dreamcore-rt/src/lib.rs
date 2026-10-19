//! Runtime mínimo para programas Rust no alvo `x86_64-unknown-dreamcore`.
//!
//! Inclui um [`allocator`] pronto (`#[global_allocator]`) sobre `mmap`, para que
//! `alloc`/`Vec`/`String`/`Box` funcionem sem o usuário escrever um alocador.
//!
//! Um binário `#![no_std]` do ImagineOS precisa de três coisas que o
//! compilador não fornece sozinho:
//!
//! 1. o símbolo `_start` (o loader entra nele, não em `main`);
//! 2. `#[panic_handler]` (o kernel não faz unwinding);
//! 3. o `memcpy`/`memset`/`memcmp`/`memmove` que o `core` e o codegen chamam.
//!
//! Esta crate entrega os três e expõe uma API mínima de syscalls. Programas
//! Rust declararam `dreamcore-rt` como dependência e definem `fn main()`
//! (ou `#[no_mangle] _start` próprio, se quiserem controle total).
//!
//! ```ignore
//! #![no_std]
// Funções `unsafe extern "C"` operam sobre ponteiros crus por contrato; a
// edition 2024 exige o bloco explícito, e a inteira crate é esse contrato.
#![allow(unsafe_op_in_unsafe_fn)]
//! #![no_main]
//! use dreamcore_rt::{entry, print, exit};
//!
//! entry!(main);
//!
//! fn main() -> ! {
//!     print("olá do Rust\n");
//!     exit(0)
//! }
//! ```
//!
//! O alvo é `static`, sem relocação dinâmica nem TLS de libc: os ponteiros
//! são absolutos e o `_start` só normaliza o frame e chama o entry.

#![no_std]
// Funções `unsafe extern "C"` operam sobre ponteiros crus por contrato.
#![allow(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

pub use imagineos_abi;

pub mod allocator;

// ---------------------------------------------------------------------------
// Syscalls
// ---------------------------------------------------------------------------

/// Executa uma syscall do contrato `int 0x80`.
///
/// Números em [`imagineos_abi`]; o retorno é o valor cru do kernel (errno
/// negativo em caso de erro), igual ao que a libc C recebe.
#[inline(always)]
pub fn syscall(number: u64, arguments: [u64; 6]) -> i64 {
    let [rdi, rsi, rdx, r10, r8, r9] = arguments;
    let result: i64;
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

/// Encerra o processo com `status`.
pub fn exit(status: i32) -> ! {
    syscall(imagineos_abi::SYS_EXIT, [status as u64, 0, 0, 0, 0, 0]);
    // O kernel nunca retorna de `exit`; se retornar, a CPU fica parada em vez
    // de executar lixo.
    loop {
        unsafe { asm!("hlt") }
    }
}

/// Escreve `bytes` no descritor `fd`.
///
/// Retorna quantos bytes foram escritos ou o errno negativo.
pub fn write(fd: u64, bytes: &[u8]) -> i64 {
    syscall(
        imagineos_abi::SYS_WRITE_FD,
        [fd, bytes.as_ptr() as u64, bytes.len() as u64, 0, 0, 0],
    )
}

/// Escreve no stdout (descritor 1).
pub fn print(text: &str) -> i64 {
    write(imagineos_abi::FD_STDOUT, text.as_bytes())
}

/// Escreve no stderr (descritor 2).
pub fn eprint(text: &str) -> i64 {
    write(imagineos_abi::FD_STDERR, text.as_bytes())
}

/// `yield`: cede a CPU ao escalonador.
pub fn yield_now() {
    syscall(imagineos_abi::SYS_YIELD, [0; 6]);
}

/// Lê o relógio monotônico em microssegundos (0 se indisponível).
pub fn monotonic_microseconds() -> u64 {
    let mut spec = imagineos_abi::Timespec::default();
    let result = syscall(
        imagineos_abi::SYS_CLOCK_GETTIME,
        [
            imagineos_abi::CLOCK_MONOTONIC,
            &mut spec as *mut _ as u64,
            0,
            0,
            0,
            0,
        ],
    );
    if result < 0 {
        return 0;
    }
    spec.seconds.max(0) as u64 * 1_000_000 + spec.nanoseconds.max(0) as u64 / 1_000
}

/// Aloca `length` bytes zerados com `mmap` anônimo.
///
/// Devolve `None` em caso de erro. A memória é liberada com [`munmap`].
pub fn mmap(length: usize) -> Option<*mut u8> {
    let result = syscall(
        imagineos_abi::SYS_MMAP,
        [
            0,
            length as u64,
            imagineos_abi::PROT_READ | imagineos_abi::PROT_WRITE,
            imagineos_abi::MAP_ANONYMOUS | imagineos_abi::MAP_PRIVATE,
            0,
            0,
        ],
    );
    if result < 0 {
        None
    } else {
        Some(result as *mut u8)
    }
}

/// Libera uma região obtida de [`mmap`].
pub fn munmap(address: *mut u8, length: usize) -> i64 {
    syscall(
        imagineos_abi::SYS_MUNMAP,
        [address as u64, length as u64, 0, 0, 0, 0],
    )
}

// ---------------------------------------------------------------------------
// Ponto de entrada e panic
// ---------------------------------------------------------------------------

/// Define o `_start` que chama `main` e sai com o valor devolvido.
///
/// Deve ser usado em um crate de binário (`#![no_std] #![no_main]`). A
/// assinatura aceita é `fn() -> i32` ou `fn() -> !`.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn _start() -> ! {
            // A pilha do usuário já está no `rsp` (o kernel a prepara). O
            // `rn` (rust name) de `$main` não pode ser usado como símbolo
            // estável, então chamamos pelo caminho.
            let code: i32 = $main();
            $crate::exit(code)
        }
    };
    ($main:path, -> !) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn _start() -> ! {
            $main()
        }
    };
}

/// Relata uma falha de alocação e encerra com 127.
///
/// `#[alloc_error_handler]` exige `#![feature(alloc_error_handler)]`, que só
/// vale no **crate do binário**; por isso o handler é instalado pelo macro
/// [`install_allocator!`], e este é o corpo compartilhado.
#[doc(hidden)]
pub fn allocation_error(layout: core::alloc::Layout) -> ! {
    // Formatação manual (`size`/`align`) sem `alloc`, que é justamente o que
    // falhou.
    let mut buffer = [0u8; 48];
    let mut length = 0;
    length += write_decimal(&mut buffer[length..], layout.size() as u64);
    buffer[length] = b'/'; length += 1;
    length += write_decimal(&mut buffer[length..], layout.align() as u64);
    eprint("falha de alocação (tamanho/alinhamento): ");
    eprint(core::str::from_utf8(&buffer[..length]).unwrap_or("?"));
    eprint("\n");
    exit(127)
}

/// Escreve `value` em decimal em `buffer`; devolve o comprimento.
pub fn write_decimal(buffer: &mut [u8], mut value: u64) -> usize {
    if value == 0 {
        buffer[0] = b'0';
        return 1;
    }
    let mut digits = [0u8; 20];
    let mut count = 0;
    while value != 0 && count < digits.len() {
        digits[count] = b'0' + (value % 10) as u8;
        value /= 10;
        count += 1;
    }
    for index in 0..count {
        buffer[index] = digits[count - 1 - index];
    }
    count
}

/// `#[panic_handler]` padrão: reporta no stderr e encerra com 127.
///
/// O C runtime do projeto usa o mesmo código, para que uma falha apareça como
/// `127` no shell em vez de travar a máquina.
#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let message = info.message().as_str().unwrap_or("panic");
    eprint("panic: ");
    eprint(message);
    eprint("\n");
    exit(127)
}

// ---------------------------------------------------------------------------
// mem* exigidos pelo `core`
// ---------------------------------------------------------------------------
//
// O `core` compila chamadas a `memcpy`/`memmove`/`memset`/`memcmp` em vez de
// inline quando o tamanho não é conhecido. Em `no_std` sem libc, esses
// símbolos precisam existir. Em `x86_64` o `rep movsb`/`rep stosb` é o caminho
// mais curto e não exige SIMD.

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut core::ffi::c_void, src: *const core::ffi::c_void, count: usize) -> *mut core::ffi::c_void {
    let dest = dest.cast::<u8>();
    let src = src.cast::<u8>();
    let mut index = 0usize;
    while index < count {
        core::ptr::write_volatile(dest.add(index), core::ptr::read_volatile(src.add(index)));
        index += 1;
    }
    dest.cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut core::ffi::c_void, src: *const core::ffi::c_void, count: usize) -> *mut core::ffi::c_void {
    let (dest, src) = (dest.cast::<u8>(), src.cast::<u8>());
    if (dest as usize) <= (src as usize) || (dest as usize) >= (src as usize) + count {
        return memcpy(dest.cast(), src.cast(), count);
    }
    let mut index = count;
    while index != 0 {
        index -= 1;
        core::ptr::write_volatile(dest.add(index), core::ptr::read_volatile(src.add(index)));
    }
    dest.cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut core::ffi::c_void, value: i32, count: usize) -> *mut core::ffi::c_void {
    let dest = dest.cast::<u8>();
    let byte = value as u8;
    let mut index = 0usize;
    while index < count {
        core::ptr::write_volatile(dest.add(index), byte);
        index += 1;
    }
    dest.cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(left: *const core::ffi::c_void, right: *const core::ffi::c_void, count: usize) -> i32 {
    let (left, right) = (left.cast::<u8>(), right.cast::<u8>());
    let mut index = 0usize;
    while index < count {
        let a = core::ptr::read_volatile(left.add(index));
        let b = core::ptr::read_volatile(right.add(index));
        if a != b {
            return if a < b { -1 } else { 1 };
        }
        index += 1;
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bcmp(left: *const core::ffi::c_void, right: *const core::ffi::c_void, count: usize) -> i32 {
    memcmp(left, right, count)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(text: *const core::ffi::c_char) -> usize {
    let text = text.cast::<u8>();
    let mut length = 0usize;
    while *text.add(length) != 0 {
        length += 1;
    }
    length
}

// Mantém a seção de stack não-executável no ELF antes do `dnulink` (o DEF não
// tem essa noção, mas o linker emite um aviso sem ela).
global_asm!(".section .note.GNU-stack,\"\",@progbits");
