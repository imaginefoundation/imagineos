//! `#[global_allocator]` do ImagineOS para programas Rust.
//!
//! O kernel oferece `mmap`/`munmap`, mas com limitações que moldam o desenho:
//!
//! * no máximo **32 faixas (VMAs)** por processo — um `mmap` por alocação
//!   esgotaria a tabela com menos de 32 `Vec`s;
//! * `MMAP_MAX_LENGTH` = 64 MiB por chamada;
//! * mapear é **preguiçoso**: as páginas nascem no primeiro toque, então
//!   reservar 4 MiB de espaço virtual não custa 4 MiB de RAM.
//!
//! Por isso o alocador abre arenas de 4 MiB e **subaloca** dentro delas. Uma
//! alocação pequena nunca toca a tabela de VMAs do kernel.
//!
//! # Layout de um bloco
//!
//! ```text
//!   bloco  (alinhado a 16)
//!   │
//!   ▼
//! +----------------+---------------------+----------------------------+
//! | BlockHeader    | padding (0..align-1)| payload .................  |
//! | total          |                     |                            |
//! | payload_offset |                     |                            |
//! +----------------+---------------------+----------------------------+
//!                                          ▲
//!                                          └─ ponteiro devolvido
//! ```
//!
//! O `payload_offset` fica logo à esquerda do payload, então `dealloc` e
//! `realloc` acham o bloco **sem mágica**: o último `usize` antes do payload
//! é sempre `payload_offset`. Blocos reciclados preservam esse campo, o que
//! permite servi-los a qualquer pedido compatível com a classe.
//!
//! # Política
//!
//! Bump por arena + lista livre por classe de potência de dois (16 B a 4 KiB).
//! Sem coalescência: a fragmentação é absorvida por arenas novas, e o custo é
//! espaço virtual, não RAM. `realloc` estende o bloco quando ele é o último da
//! arena (caso de `Vec::push`/`String::push_str`).

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;

use crate::syscall;

/// Tamanho de cada arena.
const ARENA_SIZE: usize = 4 * 1024 * 1024;

/// Alinhamento mínimo do bloco e do payload.
const MIN_ALIGN: usize = 16;

/// Classes de lista livre: 16, 32, ..., 4096 bytes.
const FREE_CLASSES: usize = 9;
const CLASS_SHIFT: usize = 4;

/// Cabeçalho de 16 bytes imediatamente antes do payload.
#[repr(C)]
struct BlockHeader {
    /// Bytes do bloco inteiro (cabeçalho + padding + payload).
    total: usize,
    /// Deslocamento do payload a partir do início do bloco. Sua posição fixa
    /// (`payload[-8]`) é o que torna `dealloc` O(1) e sem busca.
    payload_offset: usize,
}

struct SharedState(UnsafeCell<AllocatorState>);
unsafe impl Sync for SharedState {}

static ALLOCATOR: SharedState = SharedState(UnsafeCell::new(AllocatorState::new()));

struct AllocatorState {
    arena_end: usize,
    bump: usize,
    /// Nó `{ next }` escrito no início do payload de cada bloco livre.
    free: [*mut FreeNode; FREE_CLASSES],
}

impl AllocatorState {
    const fn new() -> Self {
        Self {
            arena_end: 0,
            bump: 0,
            free: [ptr::null_mut(); FREE_CLASSES],
        }
    }
}

#[repr(C)]
struct FreeNode {
    next: *mut FreeNode,
}

/// `#[global_allocator]` do ImagineOS.
pub struct DreamcoreAllocator;

impl DreamcoreAllocator {
    /// Lê `payload_offset` do último `usize` antes do payload.
    unsafe fn block_of(pointer: *mut u8) -> *mut BlockHeader {
        let offset = unsafe { ptr::read((pointer as *const usize).sub(1)) };
        unsafe { pointer.sub(offset).cast::<BlockHeader>() }
    }
}

unsafe impl GlobalAlloc for DreamcoreAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(1);
        let align = layout.align().max(MIN_ALIGN);

        // Reciclagem só para blocos pequenos com alinhamento padrão.
        if align <= MIN_ALIGN {
            if let Some(class) = class_of(size) {
                let state = unsafe { &mut *ALLOCATOR.0.get() };
                let node = state.free[class];
                if !node.is_null() {
                    state.free[class] = unsafe { (*node).next };
                    return node.cast::<u8>();
                }
            }
        }

        unsafe { allocate(size, align) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        if pointer.is_null() {
            return;
        }
        let header = unsafe { Self::block_of(pointer) };
        let total = unsafe { (*header).total };
        // `payload_offset` é 16 para o caso comum; o payload útil é o resto.
        let payload = total.saturating_sub(unsafe { (*header).payload_offset });
        let Some(class) = class_of(payload) else {
            return;
        };
        let state = unsafe { &mut *ALLOCATOR.0.get() };
        let node = pointer.cast::<FreeNode>();
        unsafe {
            ptr::write(node, FreeNode { next: state.free[class] });
        }
        state.free[class] = node;
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if pointer.is_null() {
            return unsafe {
                self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()))
            };
        }
        if new_size <= layout.size() {
            return pointer;
        }
        let state = unsafe { &mut *ALLOCATOR.0.get() };
        let header = unsafe { Self::block_of(pointer) };
        let block_start = header as usize;
        let total = unsafe { (*header).total };

        // Último bloco da arena: estende o `bump`.
        if block_start.saturating_add(total) == state.bump {
            let needed = (pointer as usize - block_start).saturating_add(new_size);
            let new_bump = block_start.saturating_add(needed);
            if new_bump <= state.arena_end {
                state.bump = new_bump;
                unsafe {
                    (*header).total = needed;
                }
                return pointer;
            }
        }

        let new_pointer = unsafe {
            self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()))
        };
        if new_pointer.is_null() {
            return ptr::null_mut();
        }
        unsafe {
            ptr::copy_nonoverlapping(pointer, new_pointer, layout.size());
            self.dealloc(pointer, layout);
        }
        new_pointer
    }
}

/// Classe de lista livre para um payload, ou `None` se é grande demais.
fn class_of(size: usize) -> Option<usize> {
    let mut class = 0usize;
    let mut limit = 1usize << CLASS_SHIFT;
    while class < FREE_CLASSES {
        if size <= limit {
            return Some(class);
        }
        limit <<= 1;
        class += 1;
    }
    None
}

fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

/// Aloca do topo da arena, abrindo uma nova se não couber.
unsafe fn allocate(size: usize, align: usize) -> *mut u8 {
    let needed = MIN_ALIGN + align.saturating_sub(1) + size;
    let state = unsafe { &mut *ALLOCATOR.0.get() };

    for _attempt in 0..2 {
        if state.arena_end != 0 {
            let block_start = align_up(state.bump, MIN_ALIGN);
            let payload_start = align_up(block_start + size_of_header(), align);
            let end = payload_start.saturating_add(size);
            if end <= state.arena_end {
                write_block(block_start, payload_start, end);
                state.bump = end;
                return payload_start as *mut u8;
            }
        }
        // Arena cheia ou inexistente: abre nova (`needed` garante que o bloco
        // cabe mesmo com o alinhamento pedido).
        let arena = ARENA_SIZE.max(needed + MIN_ALIGN);
        let Some((start, end)) = map_arena(arena) else {
            return ptr::null_mut();
        };
        state.bump = start;
        state.arena_end = end;
    }
    ptr::null_mut()
}

const fn size_of_header() -> usize {
    core::mem::size_of::<BlockHeader>()
}

/// Grava o cabeçalho do bloco em `[block_start, payload_start)`.
fn write_block(block_start: usize, payload_start: usize, end: usize) {
    let header = block_start as *mut BlockHeader;
    unsafe {
        ptr::write(
            header,
            BlockHeader {
                total: end - block_start,
                payload_offset: payload_start - block_start,
            },
        );
    }
}

/// `mmap` anônimo gravável; devolve `(início, fim)`.
fn map_arena(size: usize) -> Option<(usize, usize)> {
    let result = syscall(
        imagineos_abi::SYS_MMAP,
        [
            0,
            size as u64,
            imagineos_abi::PROT_READ | imagineos_abi::PROT_WRITE,
            imagineos_abi::MAP_ANONYMOUS | imagineos_abi::MAP_PRIVATE,
            0,
            0,
        ],
    );
    if result < 0 {
        return None;
    }
    let start = result as usize;
    Some((start, start + size))
}

/// Instala o alocador como `#[global_allocator]` **e** o handler de falha.
///
/// Use no crate raiz do binário (`#![no_std]`):
///
/// ```ignore
/// extern crate alloc;
/// dreamcore_rt::install_allocator!();
/// ```
///
/// O macro emite `#[alloc_error_handler]` junto porque o atributo só é aceito
/// no crate do binário (é `#![feature(...)]` instável).
#[macro_export]
macro_rules! install_allocator {
    () => {
        #[global_allocator]
        static GLOBAL_ALLOCATOR: $crate::allocator::DreamcoreAllocator =
            $crate::allocator::DreamcoreAllocator;

        #[alloc_error_handler]
        fn dreamcore_allocation_error(layout: core::alloc::Layout) -> ! {
            $crate::allocation_error(layout)
        }
    };
}
