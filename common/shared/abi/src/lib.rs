#![no_std]

pub mod def;
pub mod syscall;

pub use syscall::*;

pub const ABI_VERSION: u64 = 1; // Não há porque mudar a versão da ABI a cada compilação,
                                // somente se houver transição de versão estável 
                                // (v1.x -> ABI 1, v2.x ABI 2)

// Limites compartilhados entre kernel, libc C (`dreamcore.h`) e runtime Rust.
// O kernel não pode aceitar pedidos maiores que estes, e nenhum deles pode
// divergir de `common/userland/libraries/libc/include/dreamcore.h`.

/// Tamanho máximo de um `write`/`read` em descritor.
pub const MAX_IO_SIZE: usize = 4096;
/// Tamanho máximo de um pedido `SYS_WRITE` (console sem descritor).
pub const MAX_CONSOLE_WRITE: usize = 512;
/// Tamanho máximo de um caminho consultado por `*_file`/`isdir`/`isfile`.
pub const MAX_PATH_QUERY: usize = 128;
/// Tamanho máximo de um caminho mutável (`open`, `mkdir`, `pivot_root`...).
pub const MAX_MUTABLE_PATH: usize = 256;
/// Buffer máximo devolvido por `readdir`/`dmesg`/`read_file`.
pub const MAX_READ_BUFFER: usize = MAX_IO_SIZE;
/// Conteúdo máximo aceito por `write_file`.
pub const MAX_WRITE_FILE_SIZE: usize = MAX_IO_SIZE;

/// Flags do `waitpid`: `WNOHANG` — não bloqueia; devolve 0 se nenhum filho
/// terminou. Necessário para supervisores de serviços (Horust) e shells.
pub const WNOHANG: i64 = 1;

/// Superfícies de clientes registráveis por processo; o painel usa 1, um
/// cliente tipo terminal usaria outra.
pub const MAX_SURFACES: usize = 8;
/// Tamanho máximo do buffer de uma superfície, em pixels (320x200 como as
/// janelas internas do compositor).
pub const SURFACE_MAX_WIDTH: u32 = 320;
pub const SURFACE_MAX_HEIGHT: u32 = 200;
pub const SURFACE_MAX_PIXELS: usize =
    (SURFACE_MAX_WIDTH as usize) * (SURFACE_MAX_HEIGHT as usize);

/// Descrição de uma superfície de cliente, preenchida pelo chamador e
/// completada pelo kernel (`address`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceInfo {
    /// Id atribuído pelo kernel (0 se o pedido falhar).
    pub id: u32,
    pub width: u32,
    pub height: u32,
    /// Pixels por linha no buffer mapeado (pode exceder `width`).
    pub pitch: u32,
    /// Endereço virtual do buffer no processo chamador.
    pub address: u64,
}

/// Eventos dirigidos ao dono de uma superfície.
pub const SURFACE_EVENT_KEY: u32 = 1;
pub const SURFACE_EVENT_MOUSE_MOVE: u32 = 2;
pub const SURFACE_EVENT_MOUSE_BUTTON: u32 = 3;
/// O compositor moveu a janela (`x`/`y` são a nova origem na tela).
pub const SURFACE_EVENT_MOVED: u32 = 4;
/// O compositor escondeu a janela (mínima ou fechada pelo WM).
pub const SURFACE_EVENT_HIDDEN: u32 = 5;

/// Snapshot de uma superfície visível, preenchido pelo kernel em
/// `surface_snapshot`. `address` já vem mapeado no espaço do servidor.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceSnapshot {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    /// Endereço virtual do buffer no espaço do servidor de display.
    pub address: u64,
    /// Origem da superfície na tela, definida pelo dono (`surface_move`).
    pub x: i32,
    pub y: i32,
    /// Ordem de empilhamento (maior = à frente).
    pub z: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceEvent {
    /// Superfície alvo.
    pub surface: u32,
    /// Um dos `SURFACE_EVENT_*`.
    pub kind: u32,
    pub x: i32,
    pub y: i32,
    pub button: u32,
    pub pressed: u32,
    /// Caractere (para `SURFACE_EVENT_KEY`).
    pub key: u32,
}

pub const SIGNAL_HUP: u64 = 1;
pub const SIGNAL_INT: u64 = 2;
pub const SIGNAL_KILL: u64 = 9;
pub const SIGNAL_SEGV: u64 = 11;
pub const SIGNAL_TERM: u64 = 15;
pub const SIGNAL_CONT: u64 = 18;
pub const SIGNAL_STOP: u64 = 19;
pub const SIGNAL_DEFAULT: u64 = 0;
pub const SIGNAL_IGNORE: u64 = 1;

pub const PROCESS_RUNNING: u32 = 1;
pub const PROCESS_STOPPED: u32 = 2;
// Large enough for a shell plus a handful of pipeline stages. Each slot costs a
// 64 KiB kernel stack (see `KERNEL_STACKS`), so this is not free memory.
pub const MAX_PROCESSES: usize = 16;
pub const PROCESS_NAME_SIZE: usize = 64;
pub const ACCOUNT_NAME_SIZE: usize = 32;
pub const ACCOUNT_PASSWORD_SIZE: usize = 64;
pub const HOSTNAME_SIZE: usize = 64;

pub const FD_STDIN: u64 = 0;
pub const FD_STDOUT: u64 = 1;
pub const FD_STDERR: u64 = 2;
pub const MAX_OPEN_FDS: usize = 24;

pub const OPEN_READ: u64 = 1 << 0;
pub const OPEN_WRITE: u64 = 1 << 1;
pub const OPEN_CREATE: u64 = 1 << 2;
pub const OPEN_TRUNCATE: u64 = 1 << 3;
pub const OPEN_APPEND: u64 = 1 << 4;
pub const OPEN_EXCLUSIVE: u64 = 1 << 5;

pub const MAX_EXEC_ARGS: usize = 12;
pub const MAX_EXEC_ENV: usize = 12;
pub const MAX_EXEC_ITEM_SIZE: usize = 128;

/// Number of pipes available system-wide (each has a read end and a write end).
pub const MAX_PIPES: usize = 8;
/// Capacity of each pipe ring buffer, in bytes.
pub const PIPE_CAPACITY: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UserArg {
    pub address: u64,
    pub length: u64,
}

/// Describes how a child created by `SYS_SPAWN` should see its standard
/// descriptors. Each field is a descriptor number owned by the *caller*: the
/// kernel duplicates it into the child, so `0`, `1`, `2` keep meaning
/// stdin/stdout/stderr. `-1` means "leave it as the default".
///
/// This is how a shell sets up a pipeline (`child 1 | child 2`) without a real
/// `fork`: give the left child `stdout = pipe_write`, the right child
/// `stdin = pipe_read`, and close the unused ends in the parent afterwards.
/// Describes how a child created by `SYS_SPAWN` should see its standard
/// descriptors, and carries the environment for the child.
///
/// The six syscall registers are all used by `SYS_SPAWN`, so the environment
/// (pointer and count) travels inside this structure instead of taking extra
/// registers. Both fields are `u64` so the layout is identical for the kernel
/// and for 64-bit userspace.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnIo {
    pub stdin_fd: i64,
    pub stdout_fd: i64,
    pub stderr_fd: i64,
    /// Ponteiro para um array de [`UserArg`] com o ambiente do filho.
    pub environment: u64,
    /// Quantidade de entradas em `environment`.
    pub environment_count: u64,
}

impl SpawnIo {
    /// Sentinel meaning "keep the default descriptor".
    pub const INHERIT: i64 = -1;
}

/// `Default` inherits every descriptor and passes no environment.
///
/// A caller that only wants to override one descriptor writes just that field:
///
/// ```ignore
/// SpawnIo { stdout_fd: write_end as i64, ..SpawnIo::default() }
/// ```
impl Default for SpawnIo {
    fn default() -> Self {
        Self {
            stdin_fd: Self::INHERIT,
            stdout_fd: Self::INHERIT,
            stderr_fd: Self::INHERIT,
            environment: 0,
            environment_count: 0,
        }
    }
}

/// Descrição do framebuffer para o servidor de display.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FramebufferInfo {
    /// Endereço **virtual** onde o framebuffer aparece no processo chamador,
    /// preenchido por `fb_map`.
    pub address: u64,
    /// Endereço físico da memória de vídeo (informativo).
    pub physical_address: u64,
    pub width: u32,
    pub height: u32,
    /// Bytes por linha (pode ser maior que `width * 4`).
    pub pitch: u32,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
    pub reserved: u8,
}

/// Tipos de evento de entrada entregues ao servidor de display.
pub const INPUT_KEY: u32 = 1;
pub const INPUT_MOUSE_MOVE: u32 = 2;
pub const INPUT_MOUSE_BUTTON: u32 = 3;

/// A base do espaço virtual onde os buffers de superfície são mapeados. Fica
/// bem longe da heap (`USER_HEAP_LIMIT` = 64 MiB sobre `0x1_0000_0000`; a base
/// aqui é +256 MiB) e do framebuffer (`0x4000_0000_0000`). Cada superfície
/// recebe uma fatia alinhada de 1 MiB, então o endereço do cliente nunca
/// colide.
pub const SURFACE_REGION_BASE: u64 = 0x0000_0002_4000_0000;
pub const SURFACE_REGION_STRIDE: u64 = 0x10_0000; // 1 MiB

/// Um evento de entrada em formato fixo (sem ponteiros).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputEvent {
    /// Um dos `INPUT_*`.
    pub kind: u32,
    /// Tecla (para `INPUT_KEY`): o caractere ou um código de tecla especial.
    pub key: u32,
    /// Posição do cursor (para eventos de mouse).
    pub x: i32,
    pub y: i32,
    /// Botão (1 = esquerdo, 2 = direito, 3 = meio).
    pub button: u32,
    /// 1 = pressionado, 0 = liberado.
    pub pressed: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UserStat {
    pub size: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u64,
    pub state: u32,
    pub pending_signals: u32,
    pub name: [u8; PROCESS_NAME_SIZE],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstallConfig {
    pub username_length: u8,
    pub password_length: u8,
    pub hostname_length: u8,
    pub add_user: u8,
    pub administrator: u8,
    pub reserved: [u8; 3],
    pub username: [u8; ACCOUNT_NAME_SIZE],
    pub password: [u8; ACCOUNT_PASSWORD_SIZE],
    pub hostname: [u8; HOSTNAME_SIZE],
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            username_length: 0,
            password_length: 0,
            hostname_length: 0,
            add_user: 0,
            administrator: 0,
            reserved: [0; 3],
            username: [0; ACCOUNT_NAME_SIZE],
            password: [0; ACCOUNT_PASSWORD_SIZE],
            hostname: [0; HOSTNAME_SIZE],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserIdentity {
    pub uid: u32,
    pub gid: u32,
    pub is_admin: u32,
    pub username_length: u32,
    pub username: [u8; ACCOUNT_NAME_SIZE],
    pub hostname_length: u32,
    pub hostname: [u8; HOSTNAME_SIZE],
}

impl Default for UserIdentity {
    fn default() -> Self {
        Self {
            uid: 0,
            gid: 0,
            is_admin: 0,
            username_length: 0,
            username: [0; ACCOUNT_NAME_SIZE],
            hostname_length: 0,
            hostname: [0; HOSTNAME_SIZE],
        }
    }
}

impl Default for ProcessInfo {
    fn default() -> Self {
        Self {
            pid: 0,
            state: 0,
            pending_signals: 0,
            name: [0; PROCESS_NAME_SIZE],
        }
    }
}

impl UserArg {
    pub const EMPTY: Self = Self {
        address: 0,
        length: 0,
    };
}
