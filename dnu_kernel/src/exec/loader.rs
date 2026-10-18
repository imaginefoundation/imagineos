//! Carregamento de imagens executáveis **DEF** (Dreamcore Executable Format).
//!
//! O DEF é o único formato de userspace aceito: o kernel identifica a imagem
//! pelo magic `DNUF` e rejeita qualquer outra coisa (inclusive ELF) com
//! `LoadError::InvalidDef`.

use core::mem::size_of;
use core::ptr;

use crate::abi::trap::TrapFrame;
use crate::exec::def::{self, Def};
use crate::fpu;
use crate::gdt;
use crate::paging::{AddressSpace, MapError};
use crate::exec::process::{
    FpuState, Priority, Process, KERNEL_STACK_SIZE, KERNEL_STACKS, MAX_EXEC_ARGS, MAX_EXEC_ENV,
    PAGE_SIZE, USER_HEAP_START, USER_STACK_SIZE, USER_STACK_TOP,
};

use super::space::copy_to_space_result;

/// Falhas possíveis ao montar um processo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// O arquivo não é um DEF válido (magic ausente ou cabeçalho incoerente).
    InvalidDef(def::Error),
    OutOfMemory,
    InvalidMapping,
    /// `process::init` recebeu uma lista vazia de programas.
    EmptyPrograms,
}

/// Menor pilha de usuário aceita quando o binário DEF declara a sua própria.
pub const MIN_USER_STACK_SIZE: u64 = 8 * 4096;
/// Maior pilha de usuário aceita quando o binário DEF declara a sua própria.
pub const MAX_USER_STACK_SIZE: u64 = 4096 * 4096;

/// Um segmento carregável, achatado a partir do DEF.
struct LoadedSegment {
    virtual_address: u64,
    memory_size: u64,
    file_offset: usize,
    file_size: usize,
}

/// Uma imagem executável já validada (sempre DEF).
type Image<'a> = Def<'a>;

/// Pilha inicial pedida pela imagem, limitada à faixa aceita.
///
/// O DEF declara explicitamente o tamanho da pilha. Valores absurdos são
/// grampeados para que um binário malformado não esgote a memória.
fn stack_size(image: &Image<'_>) -> u64 {
    match image.stack_size {
        0 => USER_STACK_SIZE,
        requested => requested.clamp(MIN_USER_STACK_SIZE, MAX_USER_STACK_SIZE),
    }
}

fn loaded_segment(image: &Image<'_>, index: usize) -> Result<Option<LoadedSegment>, LoadError> {
    Ok(image
        .segment(index)
        .map_err(LoadError::InvalidDef)?
        .map(|segment| LoadedSegment {
            virtual_address: segment.virtual_address,
            memory_size: segment.memory_size,
            file_offset: segment.file_offset,
            file_size: segment.file_size,
        }))
}


/// Onde escrever o `TrapFrame` inicial do processo novo.
pub enum FrameLocation {
    /// Topo da pilha de kernel do slot (ninguém mais a usa).
    TopOfSlot(usize),
    /// Endereço explícito (`execve` usa a base da própria pilha).
    At(*mut TrapFrame),
}

/// Endereço na base da pilha de kernel de `slot` onde `execve` monta o frame
/// do novo programa sem sobrepor o frame da syscall em andamento.
pub fn execve_frame_scratch(slot: usize) -> Option<*mut TrapFrame> {
    let stacks = unsafe { &mut *KERNEL_STACKS.0.get() };
    let base = stacks.0[slot].as_mut_ptr() as usize;
    let offset = 2 * size_of::<TrapFrame>();
    if offset >= KERNEL_STACK_SIZE {
        return None;
    }
    Some((base + offset) as *mut TrapFrame)
}

/// Monta um `Process` a partir de uma imagem, sem registrá-lo no escalonador.
#[allow(clippy::too_many_arguments)]
pub fn load(
    image: &[u8],
    pid: usize,
    arguments: &[&[u8]],
    environment: &[&[u8]],
    allows_disk_install: bool,
    name: &str,
    location: FrameLocation,
) -> Result<Process, LoadError> {
    let image = Def::parse(image).map_err(LoadError::InvalidDef)?;
    let address_space = AddressSpace::new_user().ok_or(LoadError::OutOfMemory)?;
    map_segments(&image, &address_space)?;

    let stack_size = stack_size(&image);
    let stack_start = USER_STACK_TOP
        .checked_sub(stack_size)
        .ok_or(LoadError::InvalidMapping)?;
    let mut page = stack_start;
    while page < USER_STACK_TOP {
        address_space
            .map_user_page(page)
            .map_err(|_| LoadError::OutOfMemory)?;
        page += PAGE_SIZE;
    }
    let (user_stack, user_argv, user_envp) =
        prepare_user_stack(&address_space, arguments, environment)?;

    let entry = image.entry;
    if address_space.translate(entry).is_none() {
        return Err(LoadError::InvalidMapping);
    }

    let frame = match location {
        FrameLocation::At(pointer) => pointer,
        FrameLocation::TopOfSlot(slot) => {
            let stacks = unsafe { &mut *KERNEL_STACKS.0.get() };
            let top = stacks.0[slot].as_mut_ptr() as u64 + KERNEL_STACK_SIZE as u64;
            (top as usize - size_of::<TrapFrame>()) as *mut TrapFrame
        }
    };
    unsafe {
        frame.write(initial_frame(entry, user_stack, user_argv, user_envp, arguments, environment));
    }

    let mut process_name = [0; imagineos_abi::PROCESS_NAME_SIZE];
    let copied_name_length = name.len().min(process_name.len());
    process_name[..copied_name_length].copy_from_slice(&name.as_bytes()[..copied_name_length]);
    let mut fpu_state = FpuState::EMPTY;
    let _ = FpuState::_ALIGN_CHECK;
    unsafe {
        fpu::reset(fpu_state.as_mut_ptr());
    }
    Ok(Process {
        pid,
        active: true,
        stopped: false,
        parent: 0,
        zombie: false,
        exit_status: 0,
        waiting_for: 0,
        quantum: 0,
        // Imagens carregadas pelo kernel (PID 1) nascem como supervisor; o
        // `process::init` ajusta o nível conforme o programa. Filhos criados em
        // userspace herdam a prioridade do pai em `fork`/`spawn`.
        priority: Priority::Supervisor,
        slice_used: 0,
        allows_disk_install,
        uid: 0,
        gid: 0,
        is_admin: true,
        username: {
            let mut username = [0; imagineos_abi::ACCOUNT_NAME_SIZE];
            username[..4].copy_from_slice(b"root");
            username
        },
        username_length: 4,
        frame,
        address_space: Some(address_space),
        heap_end: USER_HEAP_START,
        name: process_name,
        pending_signals: 0,
        signal_actions: [imagineos_abi::SIGNAL_DEFAULT; 32],
        signal_restorer: 0,
        signal_frame: None,
        fpu_state,
    })
}

fn map_segments(image: &Image<'_>, address_space: &AddressSpace) -> Result<(), LoadError> {
    for index in 0..image.segment_count() {
        let Some(segment) = loaded_segment(image, index)? else {
            continue;
        };
        let segment_end = segment
            .virtual_address
            .checked_add(segment.memory_size)
            .ok_or(LoadError::InvalidMapping)?;
        let first_page = segment.virtual_address & !(PAGE_SIZE - 1);
        let last_page = segment_end
            .checked_sub(1)
            .ok_or(LoadError::InvalidMapping)?
            & !(PAGE_SIZE - 1);
        let mut page = first_page;
        while page <= last_page {
            match address_space.map_user_page(page) {
                Ok(_) | Err(MapError::AlreadyMapped) => {}
                Err(_) => return Err(LoadError::OutOfMemory),
            }
            page += PAGE_SIZE;
        }
        zero_user_range(address_space, segment.virtual_address, segment.memory_size)?;
        let file_bytes = image
            .file_bytes(segment_file(&segment))
            .ok_or(LoadError::InvalidMapping)?;
        copy_to_space_result(address_space, segment.virtual_address, file_bytes)?;
    }
    Ok(())
}

/// Reconstrói o `def::Segment` que `Def::file_bytes` espera.
fn segment_file(segment: &LoadedSegment) -> def::Segment {
    def::Segment {
        virtual_address: segment.virtual_address,
        memory_size: segment.memory_size,
        file_offset: segment.file_offset,
        file_size: segment.file_size,
        permissions: 0,
    }
}

fn initial_frame(
    entry: u64,
    user_stack: u64,
    user_argv: u64,
    user_envp: u64,
    arguments: &[&[u8]],
    environment: &[&[u8]],
) -> TrapFrame {
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
        rdx: environment.len() as u64,
        rcx: user_envp,
        rbx: 0,
        rax: 0,
        rip: entry,
        cs: gdt::USER_CODE_SELECTOR,
        // IF = 1 para o `iretq` ligar as interrupções exatamente ao entrar em
        // ring 3, já com um frame de usuário válido na pilha. Este é o único
        // ponto seguro: o caminho do kernel que monta o frame fica sem
        // interrupções.
        rflags: 0x202,
        rsp: user_stack,
        ss: gdt::USER_DATA_SELECTOR,
        rdi: arguments.len() as u64,
        rsi: user_argv,
    }
}

/// Monta a pilha inicial no padrão SysV: strings, `envp[]`, `argv[]`.
fn prepare_user_stack(
    space: &AddressSpace,
    arguments: &[&[u8]],
    environment: &[&[u8]],
) -> Result<(u64, u64, u64), LoadError> {
    if arguments.len() > MAX_EXEC_ARGS || environment.len() > MAX_EXEC_ENV {
        return Err(LoadError::InvalidMapping);
    }
    let mut stack_pointer = USER_STACK_TOP;
    let mut argument_pointers = [0u64; MAX_EXEC_ARGS];
    let mut environment_pointers = [0u64; MAX_EXEC_ENV];
    for (index, argument) in arguments.iter().enumerate() {
        stack_pointer = stack_pointer
            .checked_sub(argument.len() as u64 + 1)
            .ok_or(LoadError::InvalidMapping)?;
        copy_to_space_result(space, stack_pointer, argument)?;
        copy_to_space_result(space, stack_pointer + argument.len() as u64, &[0])?;
        argument_pointers[index] = stack_pointer;
    }
    for (index, entry) in environment.iter().enumerate() {
        stack_pointer = stack_pointer
            .checked_sub(entry.len() as u64 + 1)
            .ok_or(LoadError::InvalidMapping)?;
        copy_to_space_result(space, stack_pointer, entry)?;
        copy_to_space_result(space, stack_pointer + entry.len() as u64, &[0])?;
        environment_pointers[index] = stack_pointer;
    }

    let environment_vector_size = (environment.len() + 1)
        .checked_mul(size_of::<u64>())
        .ok_or(LoadError::InvalidMapping)? as u64;
    let environment_vector = stack_pointer
        .checked_sub(environment_vector_size)
        .ok_or(LoadError::InvalidMapping)?
        & !0xf;
    for (index, pointer) in environment_pointers
        .iter()
        .take(environment.len())
        .enumerate()
    {
        copy_to_space_result(
            space,
            environment_vector + (index * size_of::<u64>()) as u64,
            &pointer.to_le_bytes(),
        )?;
    }
    copy_to_space_result(
        space,
        environment_vector + (environment.len() * size_of::<u64>()) as u64,
        &0u64.to_le_bytes(),
    )?;

    let argument_vector_size = (arguments.len() + 1)
        .checked_mul(size_of::<u64>())
        .ok_or(LoadError::InvalidMapping)? as u64;
    stack_pointer = environment_vector
        .checked_sub(argument_vector_size)
        .ok_or(LoadError::InvalidMapping)?
        & !0xf;
    for (index, pointer) in argument_pointers.iter().take(arguments.len()).enumerate() {
        copy_to_space_result(
            space,
            stack_pointer + (index * size_of::<u64>()) as u64,
            &pointer.to_le_bytes(),
        )?;
    }
    copy_to_space_result(
        space,
        stack_pointer + (arguments.len() * size_of::<u64>()) as u64,
        &0u64.to_le_bytes(),
    )?;
    let entry_stack = stack_pointer.checked_sub(8).ok_or(LoadError::InvalidMapping)?;
    Ok((entry_stack, stack_pointer, environment_vector))
}

fn zero_user_range(space: &AddressSpace, address: u64, size: u64) -> Result<(), LoadError> {
    let mut current = address;
    let end = address.checked_add(size).ok_or(LoadError::InvalidMapping)?;
    while current < end {
        let destination = space.translate(current).ok_or(LoadError::InvalidMapping)?;
        let available = (PAGE_SIZE - (current & (PAGE_SIZE - 1))).min(end - current);
        unsafe {
            ptr::write_bytes(destination, 0, available as usize);
        }
        current += available;
    }
    Ok(())
}
