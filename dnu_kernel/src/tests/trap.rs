#![allow(dead_code)]
//! Standalone tests for the syscall trap frame and ABI constants.
//!
//! The `TrapFrame` layout is a binary contract with `arch/x86_64/idt.rs`: the
//! stub pushes registers in a fixed order and the hardware pushes the iretq
//! frame on top. These tests keep the field order honest without booting.
//!
//! Run with:
//! `rustup run stable rustc --edition 2021 --test dnu_kernel/src/tests/trap.rs -o /tmp/trap-tests && /tmp/trap-tests`

#[path = "../abi/trap.rs"]
mod trap;

use std::mem::{offset_of, size_of};
use trap::TrapFrame;

#[test]
fn trap_frame_matches_the_int_stub_push_order() {
    // Ordem dos `push` em `dreamcore_syscall_stub` (do último para o primeiro),
    // seguida do frame que o CPU empilha no `int 0x80`.
    let expected = [
        (offset_of!(TrapFrame, r15), 0),
        (offset_of!(TrapFrame, r14), 1),
        (offset_of!(TrapFrame, r13), 2),
        (offset_of!(TrapFrame, r12), 3),
        (offset_of!(TrapFrame, r11), 4),
        (offset_of!(TrapFrame, r10), 5),
        (offset_of!(TrapFrame, r9), 6),
        (offset_of!(TrapFrame, r8), 7),
        (offset_of!(TrapFrame, rbp), 8),
        (offset_of!(TrapFrame, rdi), 9),
        (offset_of!(TrapFrame, rsi), 10),
        (offset_of!(TrapFrame, rdx), 11),
        (offset_of!(TrapFrame, rcx), 12),
        (offset_of!(TrapFrame, rbx), 13),
        (offset_of!(TrapFrame, rax), 14),
        (offset_of!(TrapFrame, rip), 15),
        (offset_of!(TrapFrame, cs), 16),
        (offset_of!(TrapFrame, rflags), 17),
        (offset_of!(TrapFrame, rsp), 18),
        (offset_of!(TrapFrame, ss), 19),
    ];
    for (offset, index) in expected {
        assert_eq!(offset, index * size_of::<u64>());
    }
    assert_eq!(size_of::<TrapFrame>(), 20 * size_of::<u64>());
}

#[test]
fn syscall_arguments_use_r10_instead_of_rcx() {
    // A ABI reserva `rcx` para o endereço de retorno do `int`; o quarto
    // argumento precisa ser lido de `r10`. Este teste documenta o contrato
    // consumido por `abi::dispatch`.
    let mut frame = empty_frame();
    frame.rdi = 1;
    frame.rsi = 2;
    frame.rdx = 3;
    frame.r10 = 4;
    frame.r8 = 5;
    frame.r9 = 6;
    assert_eq!(
        [frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9],
        [1, 2, 3, 4, 5, 6]
    );
}

#[test]
fn fail_and_ok_write_the_negative_errno_contract() {
    let mut frame = empty_frame();
    frame.fail(-14);
    assert_eq!(frame.rax, (-14i64) as u64);
    frame.ok(7);
    assert_eq!(frame.rax, 7);
}

fn empty_frame() -> TrapFrame {
    TrapFrame {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        r11: 0,
        r10: 0,
        r9: 0,
        r8: 0,
        rbp: 0,
        rdi: 0,
        rsi: 0,
        rdx: 0,
        rcx: 0,
        rbx: 0,
        rax: 0,
        rip: 0,
        cs: 0,
        rflags: 0,
        rsp: 0,
        ss: 0,
    }
}
