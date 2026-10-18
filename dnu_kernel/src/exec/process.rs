//! Escalonador de processos, sinais e primitivas de processo.
//!
//! Este módulo concentra o estado e as políticas; os blocos de apoio vivem nos
//! submódulos irmãos:
//!
//! * `loader` — carregamento de imagens DEF, pilha inicial e `TrapFrame`;
//! * `descriptors` — tabela de descritores e contagem de pontas de pipe;
//! * `space` — cópia validada entre kernel e espaço de usuário.
//!
//! Não é permitido depender de `crate::abi` aqui: a ABI chama o escalonador,
//! nunca o contrário.

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr;

use crate::abi::trap::TrapFrame;
use crate::fpu;
use crate::gdt;
use crate::paging::{self, AddressSpace, MapError};
pub use imagineos_abi::UserArg;

use crate::exec::descriptors;
use crate::exec::loader::{self, FrameLocation, LoadError};
use crate::exec::space;

pub(crate) const MAX_PROCESSES: usize = imagineos_abi::MAX_PROCESSES;
// The synchronous DFS installer nests large journal transaction buffers.
pub(crate) const KERNEL_STACK_SIZE: usize = 128 * 4096;
// A full-featured shell keeps parsed commands, expanded words and redirection
// buffers on the user stack at once. 32 KiB proved too tight (a page fault at
// the top of the stack while running `echo`). After `fork`, the child inherits
// the parent's stack at whatever depth the fork happened, and the path from
// there to `execve` (pipe setup plus command expansion) needs room of its own.
// 128 KiB covers that comfortably; the space is virtual until touched.
pub(crate) const USER_STACK_SIZE: u64 = 32 * 4096;
pub const USER_STACK_TOP: u64 = 0x0000_7fff_ffff_0000;
pub const PAGE_SIZE: u64 = 4096;
pub(crate) const USER_HEAP_START: u64 = 0x0000_0001_0000_0000;
const USER_HEAP_LIMIT: u64 = 64 * 1024 * 1024;
pub const MAX_EXEC_ARGS: usize = imagineos_abi::MAX_EXEC_ARGS;
pub const MAX_EXEC_ENV: usize = imagineos_abi::MAX_EXEC_ENV;
pub const MAX_OPEN_FDS: usize = imagineos_abi::MAX_OPEN_FDS;

// A ABI e o resto do kernel usam os descritores pelo caminho curto
// `crate::process::Descriptor*`, como antes do split em módulos.
pub use crate::exec::descriptors::{Descriptor, DescriptorKind};

/// Slot do processo atualmente escalonado.
fn current_slot() -> usize {
    unsafe { (*SCHEDULER.0.get()).current }
}

#[repr(align(16))]
pub(crate) struct KernelStacks(pub(crate) [[u8; KERNEL_STACK_SIZE]; MAX_PROCESSES]);

pub(crate) struct SharedStacks(pub(crate) UnsafeCell<KernelStacks>);
unsafe impl Sync for SharedStacks {}
pub(crate) static KERNEL_STACKS: SharedStacks = SharedStacks(UnsafeCell::new(KernelStacks(
    [[0; KERNEL_STACK_SIZE]; MAX_PROCESSES],
)));

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum Priority {
    /// Trabalho de segundo plano: cede a CPU para qualquer outra prioridade.
    Idle = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    /// Supervisor do sistema (PID 1): só perde a CPU para o `Realtime`.
    Supervisor = 4,
    /// Display/servidores híbridos e trabalho crítico de latência.
    Realtime = 5,
}

impl Priority {
    /// Prioridade padrão de um processo criado por `exec`/`spawn`/`fork`.
    pub const DEFAULT: Self = Self::Normal;

    /// Quantum (em ticks do timer) de cada nível de prioridade.
    ///
    /// Prioridade maior roda **mais tempo por fatia** e é escolhida antes, então
    /// o efeito combinado é que um processo de prioridade alta progride muito
    /// mais rápido que um de prioridade baixa sob carga.
    pub const fn quantum_ticks(self) -> u64 {
        match self {
            Self::Idle => 1,
            Self::Low => 5,
            Self::Normal => 10,
            Self::High => 15,
            Self::Supervisor => 20,
            Self::Realtime => 25,
        }
    }

    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Idle,
            1 => Self::Low,
            2 => Self::Normal,
            3 => Self::High,
            4 => Self::Supervisor,
            _ => Self::Realtime,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Process {
    pub(crate) pid: usize,
    pub(crate) active: bool,
    pub(crate) stopped: bool,
    /// Processo que chamou `fork`/`spawn`. `0` significa "sem pai" (PID 1, que é
    /// criado pelo kernel). Usado por `waitpid` e pela coleta após a morte.
    pub(crate) parent: usize,
    /// Processo já terminou, mas o pai ainda não leu o código de saída.
    /// O slot e o `exit_status` ficam reservados até um `waitpid` coletar.
    pub(crate) zombie: bool,
    /// Código de saída registrado quando o processo terminou.
    pub(crate) exit_status: i32,
    /// `pid` que este processo está aguardando em `waitpid` (0 = não espera).
    /// Usado para bloquear o pai sem consumir CPU até o filho terminar.
    pub(crate) waiting_for: usize,
    /// Ticks de timer acumulados por este processo desde a última troca.
    ///
    /// É por processo de propósito: um contador global fazia o quantum reiniciar
    /// a cada troca, e dois processos acabavam se alternando a cada tick (um
    /// milissegundo) em vez de rodar o quantum inteiro — o filho do `fork`
    /// retomava sempre no mesmo `rip` e parecia travado.
    pub(crate) quantum: u64,
    /// Prioridade de escalonamento: decide quem é escolhido primeiro e quanto
    /// tempo cada processo roda por fatia (ver [`Priority`]).
    pub(crate) priority: Priority,
    /// Ticks já usados na fatia atual; zerado a cada troca de contexto.
    pub(crate) slice_used: u64,
    pub(crate) allows_disk_install: bool,
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) is_admin: bool,
    pub(crate) username: [u8; imagineos_abi::ACCOUNT_NAME_SIZE],
    pub(crate) username_length: usize,
    pub(crate) frame: *mut TrapFrame,
    pub(crate) address_space: Option<AddressSpace>,
    pub(crate) heap_end: u64,
    pub(crate) name: [u8; imagineos_abi::PROCESS_NAME_SIZE],
    pub(crate) pending_signals: u64,
    pub(crate) signal_actions: [u64; 32],
    pub(crate) signal_restorer: u64,
    pub(crate) signal_frame: Option<TrapFrame>,
    pub(crate) fpu_state: FpuState,
}

impl Process {
    pub const EMPTY: Self = Self {
        pid: 0,
        active: false,
        stopped: false,
        parent: 0,
        zombie: false,
        exit_status: 0,
        waiting_for: 0,
        quantum: 0,
        priority: Priority::DEFAULT,
        slice_used: 0,
        allows_disk_install: false,
        uid: 0,
        gid: 0,
        is_admin: true,
        username: {
            let mut name = [0; imagineos_abi::ACCOUNT_NAME_SIZE];
            name[0] = b'r';
            name[1] = b'o';
            name[2] = b'o';
            name[3] = b't';
            name
        },
        username_length: 4,
        frame: ptr::null_mut(),
        address_space: None,
        heap_end: USER_HEAP_START,
        name: [0; imagineos_abi::PROCESS_NAME_SIZE],
        pending_signals: 0,
        signal_actions: [imagineos_abi::SIGNAL_DEFAULT; 32],
        signal_restorer: 0,
        signal_frame: None,
        fpu_state: FpuState::EMPTY,
    };
}

pub(crate) struct Scheduler {
    pub(crate) processes: [Process; MAX_PROCESSES],
    pub(crate) count: usize,
    pub(crate) current: usize,
}

impl Scheduler {
    const fn new() -> Self {
        Self::empty()
    }

    pub(crate) const fn empty() -> Self {
        Self {
            processes: [Process::EMPTY; MAX_PROCESSES],
            count: 0,
            current: 0,
        }
    }
}

pub(crate) struct SharedScheduler(pub(crate) UnsafeCell<Scheduler>);
unsafe impl Sync for SharedScheduler {}
pub(crate) static SCHEDULER: SharedScheduler = SharedScheduler(UnsafeCell::new(Scheduler::new()));

pub fn init(programs: &[(&[u8], usize)]) -> Result<(), LoadError> {
    debug_log("process: initializing scheduler\n");
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    scheduler.processes = [Process::EMPTY; MAX_PROCESSES];
    scheduler.count = 0;
    scheduler.current = 0;
    descriptors::reset_all();

    for &(image, pid) in programs.iter().take(MAX_PROCESSES) {
        #[cfg(feature = "kernel-debug")]
        {
            crate::console_write("process: loading PID ");
            crate::console_write_number(pid as u64);
            crate::console_write(" DEF\n");
        }
        let slot = scheduler.count;
        let arguments: [&[u8]; 1] = [b"/sbin/init"];
        let process = load(image, pid, slot, &arguments, &[], false, "/sbin/init")?;
        scheduler.processes[scheduler.count] = process;
        scheduler.count += 1;
    }
    if scheduler.count == 0 {
        return Err(LoadError::EmptyPrograms);
    }
    Ok(())
}

/// Estado FPU/SSE de um processo (`fxsave`/`fxrstor`), alinhado a 16 bytes.
///
/// Mantido aqui porque é parte do `Process`; o loader inicializa com
/// `fpu::reset`.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(crate) struct FpuState(pub(crate) [u8; fpu::FXSAVE_SIZE]);

impl FpuState {
    pub(crate) const EMPTY: Self = Self([0; fpu::FXSAVE_SIZE]);
    pub const _ALIGN_CHECK: () = assert!(core::mem::align_of::<Self>() >= fpu::FXSAVE_ALIGN);

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
}

/// Carrega uma imagem DEF no slot indicado (usado pelo boot de PID 1).
fn load(
    image: &[u8],
    pid: usize,
    slot: usize,
    arguments: &[&[u8]],
    environment: &[&[u8]],
    allows_disk_install: bool,
    name: &str,
) -> Result<Process, LoadError> {
    loader::load(
        image,
        pid,
        arguments,
        environment,
        allows_disk_install,
        name,
        FrameLocation::TopOfSlot(slot),
    )
}



pub fn start() -> ! {
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let process = scheduler.processes[0];
    scheduler.current = 0;
    let address_space = process.address_space.expect("process address space");
    let kernel_stack_top =
        unsafe { (*KERNEL_STACKS.0.get()).0[0].as_ptr() as u64 + KERNEL_STACK_SIZE as u64 };
    gdt::set_kernel_stack(kernel_stack_top);
    paging::switch(address_space);
    unsafe {
        asm!(
            "mov rsp, {frame}",
            "mov ax, 0x33",
            "mov ds, ax",
            "mov es, ax",
            "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
            "pop r10", "pop r9", "pop r8", "pop rbp", "pop rdi",
            "pop rsi", "pop rdx", "pop rcx", "pop rbx", "pop rax",
            // The saved RFLAGS has IF=1, so this iretq atomically enables
            // interrupts while entering ring 3, with a user frame already on the
            // stack for the timer handler to preempt.
            "iretq",
            frame = in(reg) process.frame,
            options(noreturn)
        );
    }
}

pub fn current_pid() -> usize {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    scheduler.processes[scheduler.current].pid
}

/// Salva o estado FPU/SSE do processo atualmente em execução.
///
/// Deve ser chamada ao entrar no kernel (syscall/exceção), antes que código
/// Rust do kernel possa usar registradores XMM.
pub fn save_current_fpu() {
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    unsafe {
        fpu::save(scheduler.processes[scheduler.current].fpu_state.as_mut_ptr());
    }
}

/// Restaura o estado FPU/SSE do processo que vai retomar a execução.
///
/// Deve ser chamada imediatamente antes de retornar para o espaço de usuário,
/// usando o ponteiro de `TrapFrame` que será retomado.
pub fn restore_fpu_for(frame: *mut TrapFrame) {
    if frame.is_null() {
        return;
    }
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let slot = scheduler.current;
    if scheduler.processes[slot].frame == frame {
        unsafe {
            fpu::restore(scheduler.processes[slot].fpu_state.as_ptr());
        }
    }
}

pub fn current_credentials() -> (u32, u32, bool) {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    let process = scheduler.processes[scheduler.current];
    (process.uid, process.gid, process.is_admin)
}

pub fn yield_current(frame: *mut TrapFrame) -> *mut TrapFrame {
    schedule(frame, false)
}

/// Preempts the running process in response to a timer tick.
///
/// Called from the IRQ0 handler with the interrupt frame of the process that was
/// interrupted. When another runnable process exists, this switches address
/// spaces, kernel stack and FPU/SSE state (via [`schedule`]) and returns the
/// frame of the process that should resume. When nothing else is runnable it
/// returns `frame` unchanged, so the current process continues without noticing
/// the tick beyond the lost time slice.
pub fn timer_tick(frame: *mut TrapFrame) -> *mut TrapFrame {
    if frame.is_null() {
        return frame;
    }
    // Só preempta processos que estavam em **ring 3**.
    //
    // Um tick que interrompe o kernel (uma syscall em andamento, `cs & 3 == 0`)
    // não deve trocar de contexto: a syscall ficaria pendurada no meio, com a
    // pilha de kernel daquele processo "congelada" em um ponto arbitrário. Era
    // isto que impedia o `fork` de funcionar com o timer ligado — o filho era
    // preemptado dentro da própria largada e retomava sempre do mesmo `rip`,
    // sem nunca executar código de usuário.
    unsafe {
        let interrupted = &*frame;
        if interrupted.cs & 3 != 3 {
            return frame;
        }
    }
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    let current = scheduler.current;
    if !scheduler.processes[current].active {
        return frame;
    }
    // Enquanto houver alguém bloqueado em `waitpid`, não pede troca: o processo
    // que está rodando precisa consumir o tempo para produzir o evento que
    // destrava quem espera (um `exit` de filho, por exemplo). Sem isto, dois
    // `waitpid` concorrentes reescalavam um ao outro a cada tick e nenhum dos
    // dois progredia.
    if any_waiting() {
        return frame;
    }

    // Escalonamento preemptivo por prioridade: a cada tick o processo em
    // execução **consome um tick da sua fatia** e o escalonador reavalia quem
    // deve rodar. Dois motivos para trocar imediatamente:
    //
    // 1. fatia esgotada (o processo usou `quantum_ticks()` da sua prioridade);
    // 2. chegou (ou voltou a ser elegível) um processo de prioridade **maior**,
    //    que tem direito à CPU agora — é isto que torna a preempção sensível à
    //    prioridade e não apenas temporal.
    //
    // A escolha em si vive em `select_candidate`, que é a mesma função usada
    // pelas trocas voluntárias (`yield`, `exit`), então as duas rotas não podem
    // divergir.
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let current = scheduler.current;
    let priority = scheduler.processes[current].priority;
    scheduler.processes[current].slice_used += 1;
    let slice_exhausted = scheduler.processes[current].slice_used >= priority.quantum_ticks();

    let Some(candidate) = select_candidate(scheduler) else {
        // Ninguém mais elegível: o processo atual continua e a fatia zera para
        // que ele não seja preemptado no próximo tick por um contador velho.
        if slice_exhausted {
            scheduler.processes[current].slice_used = 0;
        }
        return frame;
    };
    let higher_priority = scheduler.processes[candidate].priority > priority;
    if !slice_exhausted && !higher_priority {
        return frame;
    }
    schedule(frame, false)
}

/// Escolhe o próximo processo a rodar: maior prioridade primeiro; entre
/// processos de mesma prioridade, round-robin a partir do atual.
///
/// Elegível é o processo ativo, não parado, não zumbi e não bloqueado em
/// `waitpid`. Um zumbi espera ser coletado, e um processo bloqueado em
/// `waitpid` gastaria a fatia reavaliando a mesma espera — o filho que ele
/// aguarda nunca seria escalonado no meio disso. Devolve `None` quando só o
/// processo atual é elegível.
fn select_candidate(scheduler: &Scheduler) -> Option<usize> {
    let current = scheduler.current;
    let mut best: Option<usize> = None;
    for distance in 1..=MAX_PROCESSES {
        let index = (current + distance) % MAX_PROCESSES;
        let process = &scheduler.processes[index];
        if !is_runnable(process) {
            continue;
        }
        // Desempate por round-robin: a varredura começa em `current + 1`, então
        // o primeiro de cada nível de prioridade encontrado é o que está há
        // mais tempo sem rodar.
        match best {
            Some(best_index) if scheduler.processes[best_index].priority >= process.priority => {}
            _ => best = Some(index),
        }
    }
    best
}

fn is_runnable(process: &Process) -> bool {
    process.active && !process.stopped && !process.zombie && process.waiting_for == 0
}

/// Indica se algum processo está bloqueado em `waitpid`.
///
/// O `waitpid` bloqueia o pai e o `schedule` escolhe o filho. Mas quando **o
/// próprio filho** também espera (por um neto) e o avô espera o filho, o
/// escalonador pode ficar num ciclo em que cada `waitpid` recém-retomado
/// reescala imediatamente, sem deixar o processo em execução consumir o seu
/// quantum. Enquanto houver alguém bloqueado, o timer deixa de pedir troca,
/// para que quem estiver rodando progrida até o evento que destrava os demais.
pub fn any_waiting() -> bool {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    scheduler
        .processes
        .iter()
        .any(|process| process.active && process.waiting_for != 0)
}

/// Encerra o processo atual com `status`, registrando o código para que o pai
/// possa coletá-lo em `waitpid`.
///
/// O slot vira um *zumbi*: `active` continua verdadeiro e `exit_status` fica
/// guardado até o pai chamar `waitpid`. Só então o slot é realmente liberado.
/// Isto é o comportamento do UNIX e o que permite que `$?` funcione depois que
/// o filho já morreu.
pub fn exit_current(frame: *mut TrapFrame, status: i32) -> *mut TrapFrame {
    // O init é o supervisor do sistema: encerrá-lo deixaria a máquina sem PID 1
    // (ninguém coleta zumbis, reinicia serviços ou desliga). Ver `crate::init`.
    if crate::init::protection::can_exit(current_pid()).is_err() {
        crate::console_write("init: exit refused (PID 1 is the system supervisor)\n");
        unsafe {
            (*frame).rax = crate::abi::trap::EPERM as u64;
        }
        return frame;
    }
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    scheduler.processes[scheduler.current].exit_status = status;
    schedule(frame, true)
}

pub fn terminate_faulting_process() -> *mut TrapFrame {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    let frame = scheduler.processes[scheduler.current].frame;
    if frame.is_null() {
        crate::kernel_halt();
    }
    schedule(frame, true)
}

pub fn send_signal(pid: usize, signal: u64) -> i64 {
    if !valid_signal(signal) {
        return -22;
    }
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let sender_uid = scheduler.processes[scheduler.current].uid;
    let Some(process) = scheduler
        .processes
        .iter_mut()
        .find(|process| process.active && process.pid == pid)
    else {
        return -3;
    };
    if sender_uid != 0 && sender_uid != process.uid {
        return -1;
    }
    // Nem root pode matar o init: o supervisor precisa sobreviver para
    // supervisionar. Ver `crate::init::protection`.
    if crate::init::protection::can_signal(process.pid, signal).is_err() {
        return -1;
    }
    process.pending_signals |= 1u64 << signal;
    if signal == imagineos_abi::SIGNAL_CONT || signal == imagineos_abi::SIGNAL_KILL {
        process.stopped = false;
    }
    0
}

pub fn set_signal_action(signal: u64, handler: u64, restorer: u64) -> i64 {
    if !valid_signal(signal)
        || signal == imagineos_abi::SIGNAL_KILL
        || signal == imagineos_abi::SIGNAL_STOP
        || (handler != imagineos_abi::SIGNAL_DEFAULT
            && handler != imagineos_abi::SIGNAL_IGNORE
            && (restorer == 0
                || with_current_space(|space| {
                    space.translate(handler).is_none() || space.translate(restorer).is_none()
                })))
    {
        return -22;
    }
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let process = &mut scheduler.processes[scheduler.current];
    process.signal_actions[signal as usize] = handler;
    if handler > imagineos_abi::SIGNAL_IGNORE {
        process.signal_restorer = restorer;
    }
    0
}

pub fn restore_signal_context(frame: *mut TrapFrame) -> bool {
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let process = &mut scheduler.processes[scheduler.current];
    let Some(saved) = process.signal_frame.take() else {
        return false;
    };
    unsafe {
        frame.write(saved);
    }
    true
}

pub fn deliver_pending_signal(frame: *mut TrapFrame) -> *mut TrapFrame {
    if frame.is_null() {
        return frame;
    }
    loop {
        let scheduler = unsafe { &mut *SCHEDULER.0.get() };
        let slot = scheduler.current;
        let process = &mut scheduler.processes[slot];
        if !process.active {
            return frame;
        }
        let Some(signal) = next_pending_signal(process.pending_signals) else {
            return frame;
        };
        if process.signal_frame.is_some()
            && signal != imagineos_abi::SIGNAL_KILL
            && signal != imagineos_abi::SIGNAL_STOP
        {
            return frame;
        }
        process.pending_signals &= !(1u64 << signal);
        let handler = process.signal_actions[signal as usize];
        match handler {
            imagineos_abi::SIGNAL_IGNORE
                if signal != imagineos_abi::SIGNAL_KILL && signal != imagineos_abi::SIGNAL_STOP =>
            {
                continue;
            }
            _ if signal == imagineos_abi::SIGNAL_KILL
                || (handler == imagineos_abi::SIGNAL_DEFAULT && is_terminating_signal(signal)) =>
            {
                return schedule(frame, true);
            }
            _ if signal == imagineos_abi::SIGNAL_STOP => {
                process.stopped = true;
                return schedule(frame, false);
            }
            _ if signal == imagineos_abi::SIGNAL_CONT
                && handler <= imagineos_abi::SIGNAL_IGNORE =>
            {
                continue;
            }
            _ if handler > imagineos_abi::SIGNAL_IGNORE => {
                let Some(space) = process.address_space else {
                    return schedule(frame, true);
                };
                let interrupted = unsafe { *frame };
                // Enter the handler with the SysV stack alignment and a return stub.
                let stack_top = interrupted.rsp & !0xf;
                let Some(stack_slot) = stack_top.checked_sub(8) else {
                    return schedule(frame, true);
                };
                let Some(destination) = space.translate(stack_slot) else {
                    return schedule(frame, true);
                };
                unsafe {
                    ptr::write_unaligned(destination.cast::<u64>(), process.signal_restorer);
                }
                process.signal_frame = Some(interrupted);
                unsafe {
                    (*frame).rip = handler;
                    (*frame).rdi = signal;
                    (*frame).rsp = stack_slot;
                }
                return frame;
            }
            _ => continue,
        }
    }
}

pub fn snapshot() -> ([imagineos_abi::ProcessInfo; MAX_PROCESSES], usize) {
    let (processes, count, _) = snapshot_with_count();
    (processes, count)
}

/// Como [`snapshot`], mas também devolve o índice do processo em execução.
///
/// `top` precisa distinguir o processo que está rodando agora dos demais, e essa
/// informação só existe no escalonador.
pub fn snapshot_with_count() -> ([imagineos_abi::ProcessInfo; MAX_PROCESSES], usize, usize) {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    let current = scheduler.current;
    let mut result = [imagineos_abi::ProcessInfo::default(); MAX_PROCESSES];
    let mut count = 0;
    let mut current_index = usize::MAX;
    for (index, process) in scheduler.processes.iter().enumerate() {
        if !process.active {
            continue;
        }
        if index == current {
            current_index = count;
        }
        result[count] = imagineos_abi::ProcessInfo {
            pid: process.pid as u64,
            state: process_state(process),
            pending_signals: process.pending_signals as u32,
            name: process.name,
            priority: process.priority as u32,
            running: u32::from(index == current),
            quantum_ticks: process.priority.quantum_ticks() as u32,
            _reserved: 0,
        };
        count += 1;
    }
    (result, count, current_index)
}

fn process_state(process: &Process) -> u32 {
    if process.zombie {
        imagineos_abi::PROCESS_ZOMBIE
    } else if process.stopped {
        imagineos_abi::PROCESS_STOPPED
    } else if process.waiting_for != 0 {
        imagineos_abi::PROCESS_WAITING
    } else {
        imagineos_abi::PROCESS_RUNNING
    }
}

/// Prioridade do processo em execução.
pub fn current_priority() -> u8 {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    scheduler.processes[scheduler.current].priority as u8
}

/// `schedule` que também informa se houve troca de processo.
///
/// `switched == false` só acontece quando o próprio processo atual foi o
/// escolhido (nenhum outro estava elegível). Nesse caso quem chamou continua
/// rodando e pode reavaliar a condição que o levou a ceder.
fn schedule_ex(frame: *mut TrapFrame, exiting: bool) -> (*mut TrapFrame, bool) {
    let before = unsafe { (*SCHEDULER.0.get()).current };
    let next = schedule(frame, exiting);
    let after = unsafe { (*SCHEDULER.0.get()).current };
    (next, before != after)
}

fn schedule(frame: *mut TrapFrame, exiting: bool) -> *mut TrapFrame {
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let current = scheduler.current;
    scheduler.processes[current].frame = frame;
    // Preserva os registradores FPU/SSE do processo que está saindo da CPU,
    // antes de qualquer coisa que possa tocar em XMM (o próprio kernel Rust).
    unsafe {
        fpu::save(scheduler.processes[current].fpu_state.as_mut_ptr());
    }
    if exiting {
        // O processo deixa de rodar, mas o slot fica reservado como zumbi até o
        // pai coletar o status com `waitpid`. Sem isto o código de saída se
        // perderia (e `$?` não funcionaria).
        scheduler.processes[current].zombie = true;
        scheduler.processes[current].stopped = false;
        scheduler.processes[current].waiting_for = 0;
        // Fecha os descritores do processo que terminou. Sem isto as pontas de
        // pipe entregues a um filho continuavam contadas para sempre, e um
        // leitor jamais veria EOF (`cat` girava em EAGAIN indefinidamente em
        // `ls | cat`). Também evita vazar os descritores do slot reutilizado.
        descriptors::release_all(current);
        // Libera superfícies registradas por quem morre: um painel/terminal
        // externo que termine não pode deixar uma janela fantasma no
        // compositor (o snapshot checa o dono, e aqui o slot já está vazio).
        let finished_pid = scheduler.processes[current].pid;
        crate::surface::release_pid(finished_pid);
        // Se o processo que terminou era o dono da tela (servidor de display),
        // devolve o console: sem isto o último frame desenhado pelo WM ficava
        // congelado na VGA, pois o console tinha o desenho desativado e nenhum
        // novo processo redicionava a tela. Limpar dá ao shell um console em
        // branco, e a entrada volta para o caminho de teclado do kernel.
        if crate::input::is_captured() {
            crate::input::release();
            crate::framebuffer::set_active(true);
            crate::framebuffer::clear();
        }
        // Um processo que esperava por este filho pode voltar a rodar.
        let finished = scheduler.processes[current].pid;
        for index in 0..MAX_PROCESSES {
            if scheduler.processes[index].waiting_for == finished {
                scheduler.processes[index].waiting_for = 0;
            }
        }
    }

    // A escolha é a mesma do caminho preemptivo: prioridade mais alta primeiro,
    // round-robin entre iguais. `schedule` também roda no `yield`, no `exit` e
    // quando um filho fica pronto, então manter uma única função garante que a
    // política não dependa de quem pediu a troca.
    if let Some(candidate) = select_candidate(scheduler) {
        scheduler.current = candidate;
        // Fatia nova: o processo escolhido começa com o contador de ticks zerado.
        scheduler.processes[candidate].slice_used = 0;
        let next = scheduler.processes[candidate];
        let address_space = next.address_space.expect("process address space");
        let kernel_stack_top = unsafe {
            (*KERNEL_STACKS.0.get()).0[candidate].as_ptr() as u64 + KERNEL_STACK_SIZE as u64
        };
        gdt::set_kernel_stack(kernel_stack_top);
        paging::switch(address_space);
        // Restaura o estado FPU/SSE do processo que vai entrar na CPU.
        unsafe {
            fpu::restore(scheduler.processes[candidate].fpu_state.as_ptr());
        }
        return next.frame;
    }

    crate::kernel_halt()
}

pub fn spawn_current(
    path: &str,
    arguments: &[&[u8]],
    environment: &[&[u8]],
    frame: *mut TrapFrame,
) -> *mut TrapFrame {
    // Plain `exec` keeps the default stdio: it is the historical behaviour and
    // what init/getty rely on.
    spawn_current_with_io(path, arguments, environment, &imagineos_abi::SpawnIo::default(), frame)
}

/// `execve(2)`: substitui a imagem do processo atual.
///
/// O `pid` e o slot são preservados, mas memória, pilha e frame de execução são
/// trocados pelo programa em `path`. Os descritores **não** são modificados
/// (o shell já instalou as pontas de pipe antes de chamar `execve`).
///
/// Só retorna em caso de erro: em sucesso, o `TrapFrame` volta apontando para a
/// entrada do novo programa e o processo continua a execução do zero, agora em
/// ring 3. Este é o par que falta para `fork` + `exec` virar um `exec` POSIX de
/// verdade.
pub fn execve_current(
    path: &str,
    arguments: &[&[u8]],
    environment: &[&[u8]],
    frame: *mut TrapFrame,
) -> *mut TrapFrame {
    if !has_access(path, 1) {
        unsafe {
            (*frame).rax = (-13i64) as u64;
        }
        return frame;
    }
    let Some(image) = crate::ramfs::read(path) else {
        unsafe {
            (*frame).rax = (-2i64) as u64;
        }
        return frame;
    };

    let (slot, pid, process) = {
        let scheduler = unsafe { &*SCHEDULER.0.get() };
        let slot = scheduler.current;
        let process = scheduler.processes[slot];
        (slot, process.pid, process)
    };

    // `loader::load` monta o novo `TrapFrame` no **topo** da pilha de kernel do
    // slot. Isso colide com o frame do `int 0x80` que está ativo agora, porque
    // o `execve` roda no mesmo slot do processo que vai ser substituído
    // (diferente do `spawn`, que usa outro slot). Escrevemos o frame novo na
    // **base** da pilha de kernel, longe o bastante do topo para não sobrepor
    // o frame ativo do stub de syscall.
    let Some(execve_frame) = loader::execve_frame_scratch(slot) else {
        unsafe {
            (*frame).rax = (-12i64) as u64;
        }
        return frame;
    };
    let replacement = match loader::load(
        image,
        pid,
        arguments,
        environment,
        path == "/bin/distroinstall",
        path,
        FrameLocation::At(execve_frame),
    ) {
        Ok(process) => process,
        Err(_) => {
            // Imagem que não é um DEF válido (ou sem memória): o POSIX
            // reporta `ENOEXEC` para um formato não reconhecido.
            unsafe {
                (*frame).rax = crate::abi::trap::ENOEXEC as u64;
            }
            return frame;
        }
    };

    // Preserva identidade e credenciais; só a imagem muda.
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let mut replacement: Process = replacement;
    replacement.parent = process.parent;
    replacement.uid = process.uid;
    replacement.gid = process.gid;
    replacement.is_admin = process.is_admin;
    replacement.username = process.username;
    replacement.username_length = process.username_length;
    replacement.allows_disk_install = process.allows_disk_install;
    replacement.signal_actions = process.signal_actions;
    replacement.signal_restorer = process.signal_restorer;
    let new_frame = replacement.frame;
    let new_space = replacement.address_space.expect("execve address space");
    scheduler.processes[slot] = replacement;
    // Troca o espaço de endereço **agora**: o `iretq` do stub volta direto para
    // o ponto de entrada do novo programa, sem passar pelo `schedule`. Sem
    // esta troca a CPU continuaria no espaço antigo, onde o novo programa não
    // está mapeado — o processo girava falhando instruções sem nunca chamar
    // uma syscall.
    // O frame do novo programa precisa ficar registrado no processo: se o
    // `execve` for seguido de outro `schedule` (preempção imediata, por
    // exemplo), o escalonador restauraria o frame antigo e o processo voltaria
    // a executar o código anterior — do lado de fora isso parecia um `ls`
    // girando sem nunca chamar syscall.
    scheduler.processes[slot].frame = new_frame;
    paging::switch(new_space);
    let _ = pid;
    new_frame
}

/// Creates a new process, optionally rewiring its standard descriptors.
///
/// `io` holds descriptor numbers owned by the caller: each is duplicated into
/// the child (stdin/stdout/stderr respectively), so a shell can hand a pipe end
/// to a child without a real `fork`. Fields set to
/// [`imagineos_abi::SpawnIo::INHERIT`] keep the caller's descriptor.
pub fn spawn_current_with_io(
    path: &str,
    arguments: &[&[u8]],
    environment: &[&[u8]],
    io: &imagineos_abi::SpawnIo,
    frame: *mut TrapFrame,
) -> *mut TrapFrame {
    if !has_access(path, 1) {
        unsafe {
            (*frame).rax = (-13i64) as u64;
        }
        return frame;
    }
    let Some(image) = crate::ramfs::read(path) else {
        unsafe {
            (*frame).rax = (-2i64) as u64;
        }
        return frame;
    };

    let (slot, pid, parent_slot) = {
        let scheduler = unsafe { &*SCHEDULER.0.get() };
        let Some(slot) = scheduler
            .processes
            .iter()
            .position(|process| !process.active)
        else {
            unsafe {
                (*frame).rax = (-11i64) as u64;
            }
            return frame;
        };
        let pid = scheduler
            .processes
            .iter()
            .map(|process| process.pid)
            .max()
            .unwrap_or(0)
            + 1;
        (slot, pid, scheduler.current)
    };

    // O pai é quem chamou o `spawn`, para que `waitpid` funcione no shell.
    let parent_pid = {
        let scheduler = unsafe { &*SCHEDULER.0.get() };
        scheduler.processes[parent_slot].pid
    };
    let process = match load(
        image,
        pid,
        slot,
        arguments,
        environment,
        path == "/bin/distroinstall",
        path,
    ) {
        Ok(process) => process,
        Err(_) => {
            unsafe {
                (*frame).rax = crate::abi::trap::ENOEXEC as u64;
            }
            return frame;
        }
    };
    // O filho começa com stdio padrão e recebe as redireções pedidas em `io`.
    //
    // Deliberadamente *não* herdamos o resto da tabela de descritores do pai:
    // herdar fds altos fazia cada filho segurar referências extras de pipe
    // (uma cópia em fd alto + a instalada em stdio), e a contagem nunca chegava
    // a zero quando o produtor terminava — daí `ls | cat` ficar preso em
    // EAGAIN. Fechar tudo além de 0/1/2 no `spawn` é o comportamento POSIX
    // (close-on-exec) e torna a contagem de pontas de pipe previsível.
    descriptors::prepare_spawn_table(slot, parent_slot, io);
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let parent = scheduler.processes[parent_slot];
    let mut process: Process = process;
    process.uid = parent.uid;
    process.gid = parent.gid;
    process.is_admin = parent.is_admin;
    process.username = parent.username;
    process.username_length = parent.username_length;
    process.parent = parent_pid;
    scheduler.processes[slot] = process;
    scheduler.count = scheduler.count.max(slot + 1);
    unsafe {
        (*frame).rax = pid as u64;
    }
    schedule(frame, false)
}

/// `fork(2)`: cria um processo que é uma cópia do atual.
///
/// O filho recebe:
///
/// * uma **cópia** do espaço de endereço (sem copy-on-write: cada página é
///   duplicada em um frame novo);
/// * uma cópia do `TrapFrame`, com `rax = 0` — é assim que o filho distingue-se
///   do pai, que vê o pid do filho em `rax`;
/// * uma cópia da tabela de descritores, com as referências de pipe contadas;
/// * as mesmas credenciais e o registro do pai para `waitpid`.
///
/// Devolve o frame a retomar. O escalonador continua no pai (o filho entra na
/// fila e rodará no próximo `yield`), então o valor de retorno é o frame do
/// próprio pai com `rax` ajustado.
pub fn fork_current(frame: *mut TrapFrame) -> *mut TrapFrame {
    if frame.is_null() {
        return frame;
    }
    let (slot, pid, parent_slot, parent_pid) = {
        let scheduler = unsafe { &*SCHEDULER.0.get() };
        let Some(slot) = scheduler.processes.iter().position(|process| !process.active) else {
            unsafe {
                (*frame).rax = (-11i64) as u64; // EAGAIN
            }
            return frame;
        };
        let pid = scheduler
            .processes
            .iter()
            .map(|process| process.pid)
            .max()
            .unwrap_or(0)
            + 1;
        let parent = scheduler.processes[scheduler.current];
        (slot, pid, scheduler.current, parent.pid)
    };

    let (parent_space, parent_frame) = {
        let scheduler = unsafe { &*SCHEDULER.0.get() };
        let parent = scheduler.processes[parent_slot];
        (parent.address_space, unsafe { *frame })
    };
    let Some(parent_space) = parent_space else {
        unsafe {
            (*frame).rax = (-12i64) as u64; // ENOMEM
        }
        return frame;
    };
    let Some(child_space) = parent_space.clone_user() else {
        unsafe {
            (*frame).rax = (-12i64) as u64; // ENOMEM
        }
        return frame;
    };

    // O filho retoma exatamente onde o pai estava, mas com `rax = 0`.
    let stacks = unsafe { &mut *KERNEL_STACKS.0.get() };
    let kernel_stack_top = stacks.0[slot].as_mut_ptr() as u64 + KERNEL_STACK_SIZE as u64;
    let child_frame = (kernel_stack_top as usize - size_of::<TrapFrame>()) as *mut TrapFrame;
    unsafe {
        child_frame.write(TrapFrame {
            rax: 0,
            ..parent_frame
        });
    }


    // Cópia da tabela de descritores: cada ponta de pipe compartilhada ganha
    // mais uma referência, exatamente como `dup` faria em cada fd.
    descriptors::clone_table(parent_slot, slot);

    let mut fpu_state = FpuState::EMPTY;
    unsafe {
        fpu::reset(fpu_state.as_mut_ptr());
    }

    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let parent = scheduler.processes[parent_slot];
    scheduler.processes[slot] = Process {
        pid,
        active: true,
        stopped: false,
        parent: parent_pid,
        zombie: false,
        exit_status: 0,
        waiting_for: 0,
        quantum: 0,
        // O filho herda a prioridade do pai (como o nice(2) do POSIX, que
        // preserva o nível na criação).
        priority: parent.priority,
        slice_used: 0,
        allows_disk_install: parent.allows_disk_install,
        uid: parent.uid,
        gid: parent.gid,
        is_admin: parent.is_admin,
        username: parent.username,
        username_length: parent.username_length,
        frame: child_frame,
        address_space: Some(child_space),
        heap_end: parent.heap_end,
        name: parent.name,
        pending_signals: 0,
        signal_actions: parent.signal_actions,
        signal_restorer: parent.signal_restorer,
        signal_frame: None,
        fpu_state,
    };
    scheduler.count = scheduler.count.max(slot + 1);
    // O filho começa com a fatia zerada, garantindo que ele execute o quantum
    // inteiro da sua prioridade antes de o timer pedir a troca.
    scheduler.processes[slot].quantum = 0;
    unsafe {
        (*frame).rax = pid as u64;
    }
    frame
}

/// `waitpid(pid, &status, flags)`: espera um filho terminar e coleta o status.
///
/// Comportamento implementado:
///
/// * `pid > 0` — espera especificamente aquele filho;
/// * `pid < 0` — espera qualquer filho;
/// * `pid == 0` — igual a `-1` (só filhos do mesmo processo).
///
/// Devolve o `pid` coletado e escreve `status << 8` no ponteiro do usuário,
/// como o POSIX (o código de saída ocupa os 8 bits altos). Enquanto o filho não
/// termina, o kernel marca `waiting_for` e cede a CPU — sem busy-wait, o pai só
/// volta a rodar quando o filho morre (ou por preempção normal).
///
/// Se não houver filhos correspondentes, devolve `-ECHILD` (`-10`).
/// `waitpid(pid, &status, options)`: espera um filho e coleta o status.
///
/// Suporta a flag POSIX `WNOHANG`: sem ela, o comportamento mantém o ciclo
/// EAGAIN + repetição já validado; com ela, devolve **0** (pid 0) imediatamente
/// quando nenhum filho terminou, sem ceder a CPU — o padrão que supervisores
/// de serviços (Horust) e shells de trabalho usam para varrer filhos.
pub fn waitpid_current(frame: *mut TrapFrame, pid: i64, status_pointer: u64, options: i64) -> *mut TrapFrame {
    // O slot do pai é capturado **uma vez**. Usar `scheduler.current` a cada
    // volta do laço era o bug: depois de `schedule` trocar de processo, o
    // `current` apontava para outro processo, e `waiting_for` era escrito nele.
    // Isso marcava o init e o próprio filho como "esperando pelo pid 3",
    // deixando todos inelegíveis e travando o escalonador.
    let (parent_slot, parent_pid, wanted) = {
        let scheduler = unsafe { &*SCHEDULER.0.get() };
        let parent_slot = scheduler.current;
        (parent_slot, scheduler.processes[parent_slot].pid, pid)
    };

    // Primeiro procura um corpo já pronto (zumbi) para não bloquear sem motivo.
    loop {
        let (reaped, found_any) = reap_child(parent_pid, wanted, status_pointer);
        if let Some(child_pid) = reaped {
            unsafe {
                (*frame).rax = child_pid as u64;
            }
            return frame;
        }
        if !found_any {
            // Nenhum filho corresponde ao pedido.
            unsafe {
                (*frame).rax = (-10i64) as u64; // ECHILD
            }
            return frame;
        }

        // WNOHANG: nunca bloqueia. Com filho vivo e nenhum terminado, o
        // resultado é 0 ("pid 0") conforme POSIX, sem yield.
        if options & imagineos_abi::WNOHANG != 0 {
            unsafe {
                (*frame).rax = 0;
            }
            return frame;
        }

        // Há filho vivo: marca quem esperamos e cede a CPU. O `schedule`
        // remove `waiting_for` quando o filho termina, e este laço é
        // reavaliado quando o pai for escalonado de novo. A marcação vai sempre
        // no slot do pai, capturado no início.
        {
            let scheduler = unsafe { &mut *SCHEDULER.0.get() };
            // `wanted <= 0`: espera qualquer filho; `usize::MAX` sinaliza isso e
            // o `schedule` limpa o campo quando qualquer filho termina.
            scheduler.processes[parent_slot].waiting_for = if wanted > 0 {
                wanted as usize
            } else {
                usize::MAX
            };
        }
        // O filho ainda está vivo. Devolve `EAGAIN` ao userspace e **deixa o
        // pai continuar rodando** (sem trocar de contexto aqui).
        //
        // Trocar de processo dentro do `waitpid` não funciona: o stub do
        // `int 0x80` faz `iretq` para o frame devolvido, o userspace retoma
        // logo **após** o `int 0x80` — ou seja, o laço do `waitpid` no kernel
        // seria abandonado com a pilha de kernel suja, e o pai acharia que o
        // `waitpid` terminou sem ter coletado o filho (foi assim que os zumbis
        // se acumularam até esgotar os slots).
        //
        // O caminho correto é do POSIX: o pai repete a chamada. O `yield`
        // abaixo cede a CPU para o filho progredir, e o usuário chama de novo.
        unsafe {
            (*frame).rax = (-11i64) as u64; // EAGAIN
        }
        let (next, _) = schedule_ex(frame, false);
        return next;
    }
}

/// Procura um filho de `parent_pid` (zumbi ou não) que satisfaça `wanted`.
///
/// Devolve `(pid_coletado, existe_algum_filho)`.
fn reap_child(parent_pid: usize, wanted: i64, status_pointer: u64) -> (Option<usize>, bool) {
    let mut found_any = false;
    let mut collected: Option<(usize, i32)> = None;
    {
        let scheduler = unsafe { &*SCHEDULER.0.get() };
        for process in scheduler.processes.iter() {
            if !process.active || process.parent != parent_pid {
                continue;
            }
            let matches = match wanted {
                value if value > 0 => process.pid == value as usize,
                // Qualquer filho (o POSIX distingue grupos de processo em
                // `pid == 0`, que este kernel ainda não modela).
                _ => true,
            };
            if !matches {
                continue;
            }
            found_any = true;
            if process.zombie && collected.is_none() && crate::init::protection::can_be_reaped(process.pid)
            {
                collected = Some((process.pid, process.exit_status));
            }
        }
    }


    let Some((child_pid, exit_status)) = collected else {
        return (None, found_any);
    };

    // Escreve o status no formato POSIX: código nos 8 bits altos.
    let status_value = ((exit_status as u32) & 0xff) << 8;
    if status_pointer != 0 && !copy_to_current_user(status_pointer, &status_value.to_le_bytes()) {
        // Falha ao escrever o status (ponteiro inválido): não coleta o filho,
        // para que o código de saída não se perca. O chamador recebe EFAULT.
        return (None, found_any);
    }

    // Libera o slot do zumbi.
    {
        let scheduler = unsafe { &mut *SCHEDULER.0.get() };
        if let Some(slot) = scheduler
            .processes
            .iter()
            .position(|process| process.active && process.pid == child_pid)
        {
            scheduler.processes[slot] = Process::EMPTY;
        }
    }
    (Some(child_pid), found_any)
}

fn has_access(path: &str, requested: u16) -> bool {
    let (uid, gid, _) = current_credentials();
    if uid == 0 {
        return true;
    }
    let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    let mut prefix = [0u8; imagineos_abi::MAX_MUTABLE_PATH];
    let mut length = 0usize;
    for component in parent.trim_matches('/').split('/') {
        if component.is_empty() {
            continue;
        }
        if length != 0 {
            prefix[length] = b'/';
            length += 1;
        }
        if length + component.len() > prefix.len() {
            return false;
        }
        prefix[length..length + component.len()].copy_from_slice(component.as_bytes());
        length += component.len();
        let directory = core::str::from_utf8(&prefix[..length]).unwrap_or("");
        let Some(metadata) = crate::ramfs::metadata(directory) else {
            return false;
        };
        if !metadata.is_directory
            || !crate::permissions::allows(metadata.mode, metadata.uid, metadata.gid, uid, gid, 1)
        {
            return false;
        }
    }
    let Some(metadata) = crate::ramfs::metadata(path) else {
        return false;
    };
    crate::permissions::allows(
        metadata.mode,
        metadata.uid,
        metadata.gid,
        uid,
        gid,
        requested,
    )
}

pub fn authenticate_current(username: &[u8], password: &[u8], target_uid: u32) -> i64 {
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let current = &mut scheduler.processes[scheduler.current];
    let is_self = username == &current.username[..current.username_length];
    if target_uid == 0 && is_self && current.uid != 0 {
        // sudo authenticates the caller, then elevates only an explicitly enabled account.
        if !current.is_admin || crate::usr::accounts::authenticate(username, password).is_none() {
            return -1;
        }
        current.uid = 0;
        current.gid = 0;
        current.is_admin = true;
        current.username = [0; imagineos_abi::ACCOUNT_NAME_SIZE];
        current.username[..4].copy_from_slice(b"root");
        current.username_length = 4;
        return 0;
    }
    let Some(account) = crate::usr::accounts::authenticate(username, password) else {
        return -1;
    };
    if target_uid != u32::MAX && account.uid != target_uid {
        return -1;
    }
    current.uid = account.uid;
    current.gid = account.gid;
    current.is_admin = account.administrator;
    current.username = account.username;
    current.username_length = account.username_length;
    0
}

pub fn current_identity() -> imagineos_abi::UserIdentity {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    let process = scheduler.processes[scheduler.current];
    let mut identity = imagineos_abi::UserIdentity {
        uid: process.uid,
        gid: process.gid,
        is_admin: u32::from(process.is_admin),
        username_length: process.username_length as u32,
        username: process.username,
        ..imagineos_abi::UserIdentity::default()
    };
    if let Some(hostname) = crate::ramfs::read("/etc/hostname") {
        let hostname = hostname.strip_suffix(b"\n").unwrap_or(hostname);
        let length = hostname.len().min(identity.hostname.len());
        identity.hostname[..length].copy_from_slice(&hostname[..length]);
        identity.hostname_length = length as u32;
    } else {
        identity.hostname[..9].copy_from_slice(b"imagineos");
        identity.hostname_length = 9;
    }
    identity
}

fn valid_signal(signal: u64) -> bool {
    matches!(
        signal,
        imagineos_abi::SIGNAL_HUP
            | imagineos_abi::SIGNAL_INT
            | imagineos_abi::SIGNAL_KILL
            | imagineos_abi::SIGNAL_SEGV
            | imagineos_abi::SIGNAL_TERM
            | imagineos_abi::SIGNAL_CONT
            | imagineos_abi::SIGNAL_STOP
    )
}

fn next_pending_signal(pending: u64) -> Option<u64> {
    [
        imagineos_abi::SIGNAL_KILL,
        imagineos_abi::SIGNAL_STOP,
        imagineos_abi::SIGNAL_CONT,
        imagineos_abi::SIGNAL_HUP,
        imagineos_abi::SIGNAL_INT,
        imagineos_abi::SIGNAL_SEGV,
        imagineos_abi::SIGNAL_TERM,
    ]
    .into_iter()
    .find(|signal| pending & (1u64 << signal) != 0)
}

pub fn is_terminating_signal(signal: u64) -> bool {
    matches!(
        signal,
        imagineos_abi::SIGNAL_HUP
            | imagineos_abi::SIGNAL_INT
            | imagineos_abi::SIGNAL_SEGV
            | imagineos_abi::SIGNAL_TERM
    )
}

#[cfg(test)]
mod tests {
    use super::{is_terminating_signal, next_pending_signal, valid_signal};
    use imagineos_abi::{
        SIGNAL_HUP, SIGNAL_INT, SIGNAL_KILL, SIGNAL_SEGV, SIGNAL_STOP, SIGNAL_TERM,
    };

    #[test]
    fn accepts_only_signals_implemented_by_the_kernel() {
        for signal in [
            SIGNAL_HUP,
            SIGNAL_INT,
            SIGNAL_KILL,
            SIGNAL_SEGV,
            SIGNAL_TERM,
            imagineos_abi::SIGNAL_CONT,
            SIGNAL_STOP,
        ] {
            assert!(valid_signal(signal));
        }
        assert!(!valid_signal(0));
        assert!(!valid_signal(3));
    }

    #[test]
    fn uncatchable_termination_signals_have_priority_when_queued() {
        let pending = (1u64 << SIGNAL_TERM)
            | (1u64 << SIGNAL_STOP)
            | (1u64 << SIGNAL_KILL)
            | (1u64 << SIGNAL_INT);
        assert_eq!(next_pending_signal(pending), Some(SIGNAL_KILL));
        assert_eq!(next_pending_signal(1u64 << SIGNAL_STOP), Some(SIGNAL_STOP));
    }

    #[test]
    fn termination_defaults_match_the_supported_signal_contract() {
        assert!(is_terminating_signal(SIGNAL_HUP));
        assert!(is_terminating_signal(SIGNAL_INT));
        assert!(is_terminating_signal(SIGNAL_SEGV));
        assert!(is_terminating_signal(SIGNAL_TERM));
        assert!(!is_terminating_signal(SIGNAL_STOP));
        assert!(!is_terminating_signal(imagineos_abi::SIGNAL_CONT));
    }
}

pub fn can_install_to_disk() -> bool {
    let scheduler = unsafe { &*SCHEDULER.0.get() };
    scheduler.processes[scheduler.current].allows_disk_install
}

pub fn descriptor(fd: usize) -> Option<Descriptor> {
    descriptors::get(current_slot(), fd)
}

pub fn allocate_descriptor(descriptor: Descriptor) -> Option<usize> {
    descriptors::allocate(current_slot(), descriptor)
}

pub fn update_descriptor(fd: usize, descriptor: Descriptor) -> bool {
    descriptors::update(current_slot(), fd, descriptor)
}

pub fn close_descriptor(fd: usize) -> bool {
    descriptors::close(current_slot(), fd)
}

/// `dup`: devolve um novo descritor que referencia o mesmo objeto do `fd`.
///
/// Para pipes isso incrementa a contagem da ponta, exatamente como `dup(2)`.
pub fn duplicate_descriptor(fd: usize) -> Result<usize, i64> {
    descriptors::duplicate(current_slot(), fd)
}

/// `dup2`: faz `target` referenciar o mesmo objeto de `source`, fechando o que
/// houvesse em `target`. É a peça que o shell usa para instalar as pontas de um
/// pipe em `stdin`/`stdout`.
pub fn duplicate_descriptor_to(source: usize, target: usize) -> Result<usize, i64> {
    descriptors::duplicate_to(current_slot(), source, target)
}

pub fn with_current_space<T>(operation: impl FnOnce(AddressSpace) -> T) -> T {
    space::with_current(operation)
}

pub fn copy_from_current_user(address: u64, output: &mut [u8]) -> bool {
    space::copy_from_current(address, output)
}

pub fn copy_to_current_user(address: u64, input: &[u8]) -> bool {
    space::copy_to_current(address, input)
}

pub fn grow_heap(increment: i64) -> Result<u64, i64> {
    if increment < 0 {
        return Err(-22);
    }
    let scheduler = unsafe { &mut *SCHEDULER.0.get() };
    let process = &mut scheduler.processes[scheduler.current];
    let previous = process.heap_end;
    let new_end = previous
        .checked_add(increment as u64)
        .filter(|end| *end - USER_HEAP_START <= USER_HEAP_LIMIT)
        .ok_or(-12)?;
    let first_page = previous
        .checked_add(PAGE_SIZE - 1)
        .ok_or(-12)?
        & !(PAGE_SIZE - 1);
    let page_end = new_end.checked_add(PAGE_SIZE - 1).ok_or(-12)? & !(PAGE_SIZE - 1);
    let address_space = process.address_space.ok_or(-12)?;
    let mut page = first_page;
    while page < page_end {
        match address_space.map_user_page(page) {
            Ok(_) | Err(MapError::AlreadyMapped) => {}
            Err(_) => return Err(-12),
        }
        page = page.checked_add(PAGE_SIZE).ok_or(-12)?;
    }
    process.heap_end = new_end;
    Ok(previous)
}

fn debug_log(message: &str) {
    #[cfg(feature = "kernel-debug")]
    crate::console_write(message);
    #[cfg(not(feature = "kernel-debug"))]
    let _ = message;
}
