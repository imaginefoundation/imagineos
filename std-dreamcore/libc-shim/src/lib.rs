//! Shim da crate `libc` para o ImagineOS.
//!
//! A `std` do Rust (família unix) conversa com o SO **exclusivamente** pela
//! crate `libc`. No ImagineOS não há libc do sistema: há a **musl portada**
//! (`third_party/musl-1.2.6`), cujas chamadas terminam em `int 0x80` — a ABI
//! do Dreamcore.
//!
//! Este shim substitui o crate `libc` (via `[patch.crates-io]`) e:
//!
//! * declara tipos e constantes com os valores da musl (idênticos aos do
//!   Linux, porque a musl foi escrita para eles);
//! * declara as funções como `extern "C"`, resolvidas por símbolo contra a
//!   `libc.a` da musl no link final.
//!
//! Nada é reimplementado: o comportamento (paths, `stat`, `futex`, threads,
//! sinais) é o da musl, que já fala a ABI do ImagineOS. O shim é só a
//! tradução de *nomes* e *tipos*.

#![no_std]
#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    clippy::upper_case_acronyms
)]

pub type c_char = i8;
pub type c_int = i32;
pub type c_uint = u32;
pub type c_ushort = u16;
pub type c_uchar = u8;
pub type c_long = i64;
pub type c_ulong = u64;
pub type c_void = core::ffi::c_void;
pub type size_t = usize;
pub type ssize_t = isize;
pub type off_t = i64;
pub type pid_t = i32;
pub type uid_t = u32;
pub type gid_t = u32;
pub type mode_t = u32;
pub type time_t = i64;
pub type clockid_t = i32;
pub type socklen_t = u32;
pub type nlink_t = u64;
pub type blkcnt_t = i64;
pub type blksize_t = i64;
pub type ino_t = u64;
pub type dev_t = u64;
pub type pthread_t = c_ulong;
pub type sigset_t = c_ulong;
pub type useconds_t = c_uint;

pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;
// ---------------------------------------------------------------------------
// TLS do ImagineOS
// ---------------------------------------------------------------------------

/// Descreve o bloco TLS que o kernel montou (`SYS_TLS_INFO`).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TlsInfo {
    pub fs_base: u64,
    pub size: u64,
    pub alignment: u64,
}

pub const SYS_TLS_INFO: c_long = 72;

/// Pergunta ao kernel o tamanho do bloco TLS do processo.
///
/// A `std` chama isto no início (`_start`), antes de qualquer `pthread_create`,
/// e repassa para a musl (`__dreamcore_set_main_tls`), que precisa do tamanho
/// para reservar o bloco de cada thread nova. Não altera o `%fs` atual.
pub fn tls_info() -> TlsInfo {
    let mut info = TlsInfo::default();
    let result = unsafe { syscall(SYS_TLS_INFO, &mut info as *mut TlsInfo) };
    if result < 0 {
        TlsInfo::default()
    } else {
        info
    }
}

pub const EXIT_SUCCESS: c_int = 0;
pub const EXIT_FAILURE: c_int = 1;
pub const PATH_MAX: usize = 4096;
pub const NAME_MAX: usize = 255;

// errno.
pub const EPERM: c_int = 1;
pub const ENOENT: c_int = 2;
pub const EINTR: c_int = 4;
pub const EIO: c_int = 5;
pub const ENXIO: c_int = 6;
pub const E2BIG: c_int = 7;
pub const ENOEXEC: c_int = 8;
pub const EBADF: c_int = 9;
pub const ECHILD: c_int = 10;
pub const EAGAIN: c_int = 11;
pub const ENOMEM: c_int = 12;
pub const EACCES: c_int = 13;
pub const EFAULT: c_int = 14;
pub const ENOTBLK: c_int = 15;
pub const EBUSY: c_int = 16;
pub const EEXIST: c_int = 17;
pub const EXDEV: c_int = 18;
pub const ENODEV: c_int = 19;
pub const ENOTDIR: c_int = 20;
pub const EISDIR: c_int = 21;
pub const EINVAL: c_int = 22;
pub const ENFILE: c_int = 23;
pub const EMFILE: c_int = 24;
pub const ENOSPC: c_int = 28;
pub const ESPIPE: c_int = 29;
pub const EROFS: c_int = 30;
pub const EPIPE: c_int = 32;
pub const ERANGE: c_int = 34;
pub const EDEADLK: c_int = 35;
pub const ENAMETOOLONG: c_int = 36;
pub const ENOSYS: c_int = 38;
pub const ENOTEMPTY: c_int = 39;
pub const ELOOP: c_int = 40;
pub const ENOTSUP: c_int = 95;
pub const ETIMEDOUT: c_int = 110;
pub const EOVERFLOW: c_int = 75;
pub const EWOULDBLOCK: c_int = EAGAIN;

/// Ponto de armazenamento do `errno` por thread (fornecido pela musl).
pub fn __errno_location() -> *mut c_int {
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    unsafe { __errno_location() }
}

/// Alias sem underscores: alguns fragmentos do `core`/`std` referenciam
/// `errno_location` diretamente (a musl exporta só `__errno_location`).
pub fn errno_location() -> *mut c_int {
    __errno_location()
}

/// `open`/`fcntl`.
pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;
pub const O_ACCMODE: c_int = 3;
pub const O_CREAT: c_int = 0o100;
pub const O_EXCL: c_int = 0o200;
pub const O_TRUNC: c_int = 0o1000;
pub const O_APPEND: c_int = 0o2000;
pub const O_DIRECTORY: c_int = 0o200000;
pub const O_NOFOLLOW: c_int = 0o400000;
pub const O_CLOEXEC: c_int = 0o2000000;
pub const F_GETFD: c_int = 1;
pub const F_SETFD: c_int = 2;
pub const F_GETFL: c_int = 3;
pub const F_SETFL: c_int = 4;
pub const FD_CLOEXEC: c_int = 1;

/// `lseek`.
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;

/// `mmap`.
pub const PROT_NONE: c_int = 0;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;
pub const MAP_SHARED: c_int = 0x01;
pub const MAP_PRIVATE: c_int = 0x02;
pub const MAP_FIXED: c_int = 0x10;
pub const MAP_ANONYMOUS: c_int = 0x20;
pub const MAP_FAILED: *mut c_void = usize::MAX as *mut c_void;

/// Relógio.
pub const CLOCK_REALTIME: clockid_t = 0;
pub const CLOCK_MONOTONIC: clockid_t = 1;
pub const TIMER_ABSTIME: c_int = 1;

/// `poll`.
pub const POLLIN: i16 = 0x1;
pub const POLLPRI: i16 = 0x2;
pub const POLLOUT: i16 = 0x4;
pub const POLLERR: i16 = 0x8;
pub const POLLHUP: i16 = 0x10;
pub const POLLNVAL: i16 = 0x20;

/// `wait`.
pub const WNOHANG: c_int = 1;
pub const WUNTRACED: c_int = 2;
pub const CLD_EXITED: c_int = 1;
pub const CLD_KILLED: c_int = 2;
pub const CLD_DUMPED: c_int = 3;
pub const CLD_TRAPPED: c_int = 4;
pub const CLD_STOPPED: c_int = 5;
pub const CLD_CONTINUED: c_int = 6;

/// Sinais.
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGQUIT: c_int = 3;
pub const SIGILL: c_int = 4;
pub const SIGTRAP: c_int = 5;
pub const SIGABRT: c_int = 6;
pub const SIGBUS: c_int = 7;
pub const SIGFPE: c_int = 8;
pub const SIGKILL: c_int = 9;
pub const SIGUSR1: c_int = 10;
pub const SIGSEGV: c_int = 11;
pub const SIGUSR2: c_int = 12;
pub const SIGPIPE: c_int = 13;
pub const SIGALRM: c_int = 14;
pub const SIGTERM: c_int = 15;
pub const SIGCHLD: c_int = 17;
pub const SIGCONT: c_int = 18;
pub const SIGSTOP: c_int = 19;
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;
pub const SIG_ERR: usize = usize::MAX;
pub const SA_SIGINFO: c_int = 4;
pub const SA_RESTART: c_int = 0x1000_0000;
pub const SIG_BLOCK: c_int = 0;
pub const SIG_UNBLOCK: c_int = 1;
pub const SIG_SETMASK: c_int = 2;

/// `stat`.
pub const S_IFMT: mode_t = 0o170000;
pub const S_IFDIR: mode_t = 0o040000;
pub const S_IFREG: mode_t = 0o100000;
pub const S_IFLNK: mode_t = 0o120000;
pub const S_IFCHR: mode_t = 0o020000;
pub const S_IFBLK: mode_t = 0o060000;
pub const S_IFIFO: mode_t = 0o010000;
pub const S_IFSOCK: mode_t = 0o140000;
pub const S_IRUSR: mode_t = 0o400;
pub const S_IWUSR: mode_t = 0o200;
pub const S_IXUSR: mode_t = 0o100;
pub const S_IRWXU: mode_t = 0o700;
pub const S_IRGRP: mode_t = 0o040;
pub const S_IWGRP: mode_t = 0o020;
pub const S_IXGRP: mode_t = 0o010;
pub const S_IRWXG: mode_t = 0o070;
pub const S_IROTH: mode_t = 0o004;
pub const S_IWOTH: mode_t = 0o002;
pub const S_IXOTH: mode_t = 0o001;
pub const S_IRWXO: mode_t = 0o007;
pub const S_ISUID: mode_t = 0o4000;
pub const S_ISGID: mode_t = 0o2000;
pub const S_ISVTX: mode_t = 0o1000;

/// `sysconf`.
pub const _SC_NPROCESSORS_ONLN: c_int = 84;
pub const _SC_PAGESIZE: c_int = 30;
pub const _SC_THREAD_STACK_MIN: c_int = 75;
pub const _SC_OPEN_MAX: c_int = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct timespec {
    pub tv_sec: time_t,
    pub tv_nsec: c_long,
}

impl Default for timespec {
    fn default() -> Self {
        Self { tv_sec: 0, tv_nsec: 0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct timeval {
    pub tv_sec: time_t,
    pub tv_usec: c_long,
}

impl Default for timeval {
    fn default() -> Self {
        Self { tv_sec: 0, tv_usec: 0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct tm {
    pub tm_sec: c_int,
    pub tm_min: c_int,
    pub tm_hour: c_int,
    pub tm_mday: c_int,
    pub tm_mon: c_int,
    pub tm_year: c_int,
    pub tm_wday: c_int,
    pub tm_yday: c_int,
    pub tm_isdst: c_int,
    pub tm_gmtoff: c_long,
    pub tm_zone: *const c_char,
}

/// Estruturas de `stat` (x86_64, layout da musl).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct stat {
    pub st_dev: dev_t,
    pub st_ino: ino_t,
    pub st_nlink: nlink_t,
    pub st_mode: mode_t,
    pub st_uid: uid_t,
    pub st_gid: gid_t,
    pub __pad0: c_int,
    pub st_rdev: dev_t,
    pub st_size: off_t,
    pub st_blksize: blksize_t,
    pub st_blocks: blkcnt_t,
    pub st_atime: time_t,
    pub st_atime_nsec: c_long,
    pub st_mtime: time_t,
    pub st_mtime_nsec: c_long,
    pub st_ctime: time_t,
    pub st_ctime_nsec: c_long,
    pub __unused: [c_long; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct statvfs {
    pub f_bsize: c_ulong,
    pub f_frsize: c_ulong,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_favail: u64,
    pub f_fsid: c_ulong,
    pub f_flag: c_ulong,
    pub f_namemax: c_ulong,
    pub __f_spare: [c_int; 6],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct pollfd {
    pub fd: c_int,
    pub events: i16,
    pub revents: i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct pthread_attr_t {
    pub __size: [u64; 7],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct sched_param {
    pub sched_priority: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct rlimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct pthread_mutex_t {
    pub __u: [u64; 5],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct pthread_mutexattr_t {
    pub __u: [c_int; 1],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct pthread_cond_t {
    pub __u: [u64; 6],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct pthread_condattr_t {
    pub __u: [c_int; 1],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct dirent {
    pub d_ino: ino_t,
    pub d_off: off_t,
    pub d_reclen: c_ushort,
    pub d_type: u8,
    pub d_name: [c_char; 256],
}

pub type pthread_key_t = c_uint;
pub type pthread_once_t = c_int;

pub const PTHREAD_MUTEX_INITIALIZER: pthread_mutex_t = pthread_mutex_t { __u: [0; 5] };
pub const PTHREAD_COND_INITIALIZER: pthread_cond_t = pthread_cond_t { __u: [0; 6] };
pub const PTHREAD_MUTEX_NORMAL: c_int = 0;
pub const PTHREAD_MUTEX_RECURSIVE: c_int = 1;
pub const PTHREAD_MUTEX_ERRORCHECK: c_int = 2;
pub const PTHREAD_ONCE_INIT: pthread_once_t = 0;
pub const PTHREAD_CREATE_DETACHED: c_int = 1;
pub const PTHREAD_CREATE_JOINABLE: c_int = 0;

/// Módulos que a `std` referencia como `libc::unistd`, `libc::pthread` etc.
/// Reexportam os mesmos símbolos no namespace do crate, que é como o `extern
/// "C"` os resolve no link (a musl exporta esses nomes globais).
pub mod unistd {
    pub use super::{
        _exit, close, dup, dup2, fork, fsync, getpid, getppid, isatty, lseek, read, readlink,
        rmdir, sethostname, sleep, sysconf, unlink, write,
    };
}

pub mod stdio {
    pub use super::{fclose, fflush, fopen, fprintf, fwrite};
}

pub mod pthread {
    pub use super::{
        pthread_attr_destroy, pthread_attr_init, pthread_attr_setdetachstate,
        pthread_attr_setguardsize, pthread_attr_setstacksize, pthread_cond_broadcast,
        pthread_cond_destroy, pthread_cond_init, pthread_cond_signal, pthread_cond_timedwait,
        pthread_cond_wait, pthread_create, pthread_detach, pthread_equal, pthread_getattr_np,
        pthread_getname_np, pthread_join, pthread_key_create, pthread_key_delete,
        pthread_mutex_destroy, pthread_mutex_init, pthread_mutex_lock, pthread_mutex_trylock,
        pthread_mutex_unlock, pthread_once, pthread_self, pthread_setname_np,
        pthread_setschedparam, pthread_sigmask,
    };
}

pub mod errno {
    pub use super::__errno_location;
}

// ---------------------------------------------------------------------------
// Declarações das funções (resolvidas contra a musl no link).
//
// Declaradas no bloco `unsafe extern "C"` do módulo raiz: é a forma que faz
// `libc::open`, `libc::signal`, `libc::pread` etc. existirem como itens
// importáveis pela `std`. (Um glob de um bloco `extern` aninhado não expõe os
// nomes.)
// ---------------------------------------------------------------------------

unsafe extern "C" {
    // processo e arquivo
    pub fn _exit(status: c_int) -> !;
    pub fn exit(status: c_int) -> !;
    pub fn abort() -> !;
    pub fn getpid() -> pid_t;
    pub fn getppid() -> pid_t;
    pub fn fork() -> pid_t;
    pub fn close(fd: c_int) -> c_int;
    pub fn dup(fd: c_int) -> c_int;
    pub fn dup2(from: c_int, to: c_int) -> c_int;
    pub fn read(fd: c_int, buffer: *mut c_void, count: size_t) -> ssize_t;
    pub fn write(fd: c_int, buffer: *const c_void, count: size_t) -> ssize_t;
    pub fn lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t;
    pub fn fsync(fd: c_int) -> c_int;
    pub fn isatty(fd: c_int) -> c_int;
    pub fn unlink(path: *const c_char) -> c_int;
    pub fn rmdir(path: *const c_char) -> c_int;
    pub fn readlink(path: *const c_char, buffer: *mut c_char, size: size_t) -> ssize_t;
    pub fn sethostname(name: *const c_char, size: size_t) -> c_int;
    pub fn sysconf(name: c_int) -> c_long;
    pub fn sleep(seconds: c_uint) -> c_uint;
    pub fn getcwd(buffer: *mut c_char, size: size_t) -> *mut c_char;
    pub fn chdir(path: *const c_char) -> c_int;
    pub fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    pub fn openat(dirfd: c_int, path: *const c_char, flags: c_int, ...) -> c_int;
    pub fn unlinkat(dirfd: c_int, path: *const c_char, flags: c_int) -> c_int;
    pub fn rename(from: *const c_char, to: *const c_char) -> c_int;
    pub fn renameat(fromfd: c_int, from: *const c_char, tofd: c_int, to: *const c_char) -> c_int;
    pub fn mkdirat(dirfd: c_int, path: *const c_char, mode: mode_t) -> c_int;
    pub fn fchmod(fd: c_int, mode: mode_t) -> c_int;
    pub fn futimens(fd: c_int, times: *const timespec) -> c_int;
    pub fn utimensat(fd: c_int, path: *const c_char, times: *const timespec, flags: c_int) -> c_int;
    pub fn pread(fd: c_int, buffer: *mut c_void, count: size_t, offset: off_t) -> ssize_t;
    pub fn pwrite(fd: c_int, buffer: *const c_void, count: size_t, offset: off_t) -> ssize_t;
    pub fn readdir(dir: *mut c_void) -> *mut dirent;
    pub fn opendir(path: *const c_char) -> *mut c_void;
    pub fn closedir(dir: *mut c_void) -> c_int;
    pub fn getenv(name: *const c_char) -> *const c_char;
    pub fn signal(number: c_int, handler: usize) -> usize;
    pub fn sigemptyset(set: *mut sigset_t) -> c_int;
    pub fn sigaddset(set: *mut sigset_t, signal: c_int) -> c_int;
    pub fn sigaction(number: c_int, action: *const c_void, old: *mut c_void) -> c_int;
    pub fn syscall(number: c_long, ...) -> c_long;
    pub fn getauxval(kind: c_ulong) -> c_ulong;
    pub fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    pub fn stat(path: *const c_char, buffer: *mut stat) -> c_int;
    pub fn fstat(fd: c_int, buffer: *mut stat) -> c_int;
    pub fn lstat(path: *const c_char, buffer: *mut stat) -> c_int;
    pub fn mkdir(path: *const c_char, mode: mode_t) -> c_int;
    pub fn access(path: *const c_char, mode: c_int) -> c_int;
    pub fn statvfs(path: *const c_char, buffer: *mut statvfs) -> c_int;
    pub fn fstatvfs(fd: c_int, buffer: *mut statvfs) -> c_int;
    pub fn poll(fds: *mut pollfd, count: c_ulong, timeout: c_int) -> c_int;
    // stat em variantes: a musl expõe `fstatat`/`newfstatat`.
    #[link_name = "fstatat"]
    pub fn fstatat(dirfd: c_int, path: *const c_char, buffer: *mut stat, flags: c_int) -> c_int;
    #[link_name = "lstat"]
    pub fn lstat64(path: *const c_char, buffer: *mut stat) -> c_int;

    pub fn kill(pid: pid_t, signal: c_int) -> c_int;
    pub fn raise(signal: c_int) -> c_int;

    // stdio (a std usa para o stream de erro do panic)
    pub fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void;
    pub fn fclose(stream: *mut c_void) -> c_int;
    pub fn fflush(stream: *mut c_void) -> c_int;
    pub fn fwrite(ptr: *const c_void, size: size_t, count: size_t, stream: *mut c_void) -> size_t;
    pub fn fprintf(stream: *mut c_void, format: *const c_char, ...) -> c_int;

    // relógio
    pub fn clock_gettime(id: clockid_t, tp: *mut timespec) -> c_int;
    pub fn nanosleep(req: *const timespec, rem: *mut timespec) -> c_int;
    pub fn gettimeofday(tv: *mut timeval, tz: *mut c_void) -> c_int;
    pub fn localtime_r(time: *const time_t, result: *mut tm) -> *mut tm;
    pub fn addrinfo_t_placeholder() -> c_int;

    // memória
    pub fn mmap(
        addr: *mut c_void,
        len: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        off: off_t,
    ) -> *mut c_void;
    pub fn munmap(addr: *mut c_void, len: size_t) -> c_int;
    pub fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;
    pub fn madvise(addr: *mut c_void, len: size_t, advice: c_int) -> c_int;
    pub fn sbrk(increment: isize) -> *mut c_void;

    // threads
    pub fn pthread_create(
        thread: *mut pthread_t,
        attr: *const pthread_attr_t,
        start: extern "C" fn(*mut c_void) -> *mut c_void,
        arg: *mut c_void,
    ) -> c_int;
    pub fn pthread_join(thread: pthread_t, result: *mut *mut c_void) -> c_int;
    pub fn pthread_detach(thread: pthread_t) -> c_int;
    pub fn pthread_self() -> pthread_t;
    pub fn pthread_equal(a: pthread_t, b: pthread_t) -> c_int;
    pub fn pthread_exit(result: *mut c_void) -> !;
    pub fn pthread_attr_init(attr: *mut pthread_attr_t) -> c_int;
    pub fn pthread_attr_destroy(attr: *mut pthread_attr_t) -> c_int;
    pub fn pthread_attr_setstacksize(attr: *mut pthread_attr_t, size: size_t) -> c_int;
    pub fn pthread_attr_setguardsize(attr: *mut pthread_attr_t, size: size_t) -> c_int;
    pub fn pthread_attr_setdetachstate(attr: *mut pthread_attr_t, state: c_int) -> c_int;
    pub fn pthread_getattr_np(thread: pthread_t, attr: *mut pthread_attr_t) -> c_int;
    pub fn pthread_setname_np(thread: pthread_t, name: *const c_char) -> c_int;
    pub fn pthread_getname_np(thread: pthread_t, name: *mut c_char, size: size_t) -> c_int;
    pub fn pthread_setschedparam(
        thread: pthread_t,
        policy: c_int,
        param: *const sched_param,
    ) -> c_int;
    pub fn pthread_sigmask(how: c_int, set: *const sigset_t, old: *mut sigset_t) -> c_int;
    pub fn pthread_key_create(
        key: *mut pthread_key_t,
        destructor: Option<extern "C" fn(*mut c_void)>,
    ) -> c_int;
    pub fn pthread_key_delete(key: pthread_key_t) -> c_int;
    pub fn pthread_once(once: *mut pthread_once_t, init: extern "C" fn()) -> c_int;
    pub fn pthread_mutex_init(
        mutex: *mut pthread_mutex_t,
        attr: *const pthread_mutexattr_t,
    ) -> c_int;
    pub fn pthread_mutex_destroy(mutex: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_lock(mutex: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_cond_init(cond: *mut pthread_cond_t, attr: *const pthread_condattr_t) -> c_int;
    pub fn pthread_cond_destroy(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_cond_wait(cond: *mut pthread_cond_t, mutex: *mut pthread_mutex_t) -> c_int;
    pub fn pthread_cond_timedwait(
        cond: *mut pthread_cond_t,
        mutex: *mut pthread_mutex_t,
        timeout: *const timespec,
    ) -> c_int;
    pub fn pthread_cond_signal(cond: *mut pthread_cond_t) -> c_int;
    pub fn pthread_cond_broadcast(cond: *mut pthread_cond_t) -> c_int;

    // --- acrescentados a partir dos erros reais da std ---
    pub fn fdopendir(fd: c_int) -> *mut c_void;
    pub fn dirfd(dir: *mut DIR) -> c_int;
    pub fn ftruncate(fd: c_int, length: off_t) -> c_int;
    pub fn truncate(path: *const c_char, length: off_t) -> c_int;
    pub fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    pub fn pipe(fds: *mut c_int) -> c_int;
    pub fn dup3(from: c_int, to: c_int, flags: c_int) -> c_int;
    pub fn sendfile(out: c_int, input: c_int, offset: *mut off_t, count: size_t) -> ssize_t;
    pub fn readlinkat(fd: c_int, path: *const c_char, buf: *mut c_char, size: size_t) -> ssize_t;
    pub fn fchdir(fd: c_int) -> c_int;
    pub fn fdatasync(fd: c_int) -> c_int;
    pub fn linkat(fromfd: c_int, from: *const c_char, tofd: c_int, to: *const c_char, flags: c_int) -> c_int;
    pub fn symlinkat(target: *const c_char, fd: c_int, path: *const c_char) -> c_int;
    pub fn lchown(path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    pub fn chown(path: *const c_char, uid: uid_t, gid: gid_t) -> c_int;
    pub fn fchown(fd: c_int, uid: uid_t, gid: gid_t) -> c_int;
    pub fn getuid() -> uid_t;
    pub fn geteuid() -> uid_t;
    pub fn getgid() -> gid_t;
    pub fn getegid() -> gid_t;
    pub fn setgroups(count: size_t, groups: *const gid_t) -> c_int;
    pub fn umask(mask: mode_t) -> mode_t;
    pub fn chmod(path: *const c_char, mode: mode_t) -> c_int;
    pub fn fchmodat(fd: c_int, path: *const c_char, mode: mode_t, flags: c_int) -> c_int;
    pub fn mkfifo(path: *const c_char, mode: mode_t) -> c_int;
    pub fn realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char;
    pub fn getdtablesize() -> c_int;
    pub fn getrlimit(resource: c_int, limit: *mut rlimit) -> c_int;
    pub fn setrlimit(resource: c_int, limit: *const rlimit) -> c_int;
    pub fn getentropy(buffer: *mut c_void, length: size_t) -> c_int;
    pub fn setenv(name: *const c_char, value: *const c_char, overwrite: c_int) -> c_int;
    pub fn unsetenv(name: *const c_char) -> c_int;
    pub fn environ() -> *mut *mut c_char;
    pub fn brk(address: *mut c_void) -> *mut c_void;
    pub fn mremap(old: *mut c_void, old_size: size_t, new_size: size_t, flags: c_int, new_address: *mut c_void) -> *mut c_void;
    pub fn clock_getres(id: clockid_t, tp: *mut timespec) -> c_int;
    pub fn timespec_get(tp: *mut timespec, base: c_int) -> c_int;
    pub fn tzset();
    pub fn __syscall_ret(result: c_ulong) -> c_long;
    // --- funções adicionais usadas pela std ---
    pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    pub fn calloc(count: size_t, size: size_t) -> *mut c_void;
    pub fn malloc(size: size_t) -> *mut c_void;
    pub fn realloc(ptr: *mut c_void, size: size_t) -> *mut c_void;
    pub fn free(ptr: *mut c_void);
    pub fn posix_memalign(memptr: *mut *mut c_void, align: size_t, size: size_t) -> c_int;
    pub fn readv(fd: c_int, iov: *const iovec, count: c_int) -> ssize_t;
    pub fn writev(fd: c_int, iov: *const iovec, count: c_int) -> ssize_t;
    pub fn execvp(file: *const c_char, argv: *const *const c_char) -> c_int;
    pub fn chroot(path: *const c_char) -> c_int;
    pub fn symlink(target: *const c_char, link: *const c_char) -> c_int;
    pub fn gethostname(name: *mut c_char, len: size_t) -> c_int;
    pub fn getpwuid_r(uid: uid_t, pwd: *mut passwd, buf: *mut c_char, len: size_t, result: *mut *mut passwd) -> c_int;
    pub fn getaddrinfo(node: *const c_char, service: *const c_char, hints: *const addrinfo, result: *mut *mut addrinfo) -> c_int;
    pub fn freeaddrinfo(res: *mut addrinfo);
    pub fn gai_strerror(code: c_int) -> *const c_char;
    pub fn killpg(pgrp: pid_t, signal: c_int) -> c_int;
    pub fn waitpid(pid: pid_t, status: *mut c_int, options: c_int) -> pid_t;
    pub fn sched_yield() -> c_int;
    pub fn setuid(uid: uid_t) -> c_int;
    pub fn setgid(gid: gid_t) -> c_int;
    pub fn setpgid(pid: pid_t, pgrp: pid_t) -> c_int;
    pub fn setsid() -> pid_t;
    pub fn strnlen(text: *const c_char, max: size_t) -> size_t;
    pub fn pthread_mutexattr_init(attr: *mut pthread_mutexattr_t) -> c_int;
    pub fn pthread_mutexattr_destroy(attr: *mut pthread_mutexattr_t) -> c_int;
    pub fn pthread_mutexattr_settype(attr: *mut pthread_mutexattr_t, kind: c_int) -> c_int;
    pub fn pthread_condattr_init(attr: *mut pthread_condattr_t) -> c_int;
    pub fn pthread_condattr_destroy(attr: *mut pthread_condattr_t) -> c_int;
    pub fn pthread_condattr_setclock(attr: *mut pthread_condattr_t, id: clockid_t) -> c_int;
    pub fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;
    pub fn pthread_getspecific(key: pthread_key_t) -> *mut c_void;
    pub fn getentropy_fill(buffer: *mut c_void, length: size_t) -> c_int;

    // rede (o `sys::net` do unix compila as assinaturas mesmo sem suporte do
    // kernel; as chamadas falham em runtime com ENOSYS — ver a ABI).
    pub fn socketpair(domain: c_int, kind: c_int, protocol: c_int, fds: *mut c_int) -> c_int;
    pub fn bind(fd: c_int, addr: *const sockaddr, len: socklen_t) -> c_int;
    pub fn connect(fd: c_int, addr: *const sockaddr, len: socklen_t) -> c_int;
    pub fn listen(fd: c_int, backlog: c_int) -> c_int;
    pub fn accept(fd: c_int, addr: *mut sockaddr, len: *mut socklen_t) -> c_int;
    pub fn getsockname(fd: c_int, addr: *mut sockaddr, len: *mut socklen_t) -> c_int;
    pub fn getpeername(fd: c_int, addr: *mut sockaddr, len: *mut socklen_t) -> c_int;
    pub fn setsockopt(fd: c_int, level: c_int, name: c_int, value: *const c_void, len: socklen_t) -> c_int;
    pub fn getsockopt(fd: c_int, level: c_int, name: c_int, value: *mut c_void, len: *mut socklen_t) -> c_int;
    pub fn send(fd: c_int, buf: *const c_void, len: size_t, flags: c_int) -> ssize_t;
    pub fn recv(fd: c_int, buf: *mut c_void, len: size_t, flags: c_int) -> ssize_t;
    pub fn sendto(fd: c_int, buf: *const c_void, len: size_t, flags: c_int, addr: *const sockaddr, addrlen: socklen_t) -> ssize_t;
    pub fn recvfrom(fd: c_int, buf: *mut c_void, len: size_t, flags: c_int, addr: *mut sockaddr, addrlen: *mut socklen_t) -> ssize_t;
    pub fn shutdown(fd: c_int, how: c_int) -> c_int;

}

/// Rede: tipos e constantes mínimas (o kernel não tem pilha TCP/IP; as
/// chamadas existem para o código compilar e falham com `ENOSYS`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sockaddr {
    pub sa_family: u16,
    pub sa_data: [c_char; 14],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct sockaddr_storage {
    pub ss_family: sa_family_t,
    pub __ss_padding: [u8; 118],
    pub __ss_align: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct linger {
    pub l_onoff: c_int,
    pub l_linger: c_int,
}
pub const AF_UNIX: c_int = 1;
pub const AF_INET: c_int = 2;
pub const AF_INET6: c_int = 10;
pub const SOCK_STREAM: c_int = 1;
pub const SOCK_DGRAM: c_int = 2;
pub const SOCK_SEQPACKET: c_int = 5;
pub const SOCK_NONBLOCK: c_int = 0o4000;
pub const SOCK_CLOEXEC: c_int = 0o2000000;
pub const SOL_SOCKET: c_int = 1;
pub const SO_LINGER: c_int = 13;
pub const SO_ERROR: c_int = 4;
pub const SO_RCVTIMEO: c_int = 20;
pub const SO_SNDTIMEO: c_int = 21;
pub const SO_TYPE: c_int = 3;
pub const MSG_PEEK: c_int = 2;
pub const MSG_DONTWAIT: c_int = 0x40;
pub const MSG_NOSIGNAL: c_int = 0x4000;
pub const SHUT_RD: c_int = 0;
pub const SHUT_WR: c_int = 1;
pub const SHUT_RDWR: c_int = 2;
pub const EAI_SOCKTYPE: c_int = -7;
pub const EAI_SYSTEM: c_int = -11;

// --- tipos e constantes descobertos pelos erros reais da std ---
pub type sa_family_t = u16;
pub type uintptr_t = usize;
pub type DIR = c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct iovec {
    pub iov_base: *mut c_void,
    pub iov_len: size_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct sockaddr_un {
    pub sun_family: sa_family_t,
    pub sun_path: [c_char; 108],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct in_addr {
    pub s_addr: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct in6_addr {
    pub s6_addr: [u8; 16],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct sockaddr_in {
    pub sin_family: sa_family_t,
    pub sin_port: u16,
    pub sin_addr: in_addr,
    pub sin_zero: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct sockaddr_in6 {
    pub sin6_family: sa_family_t,
    pub sin6_port: u16,
    pub sin6_flowinfo: u32,
    pub sin6_addr: in6_addr,
    pub sin6_scope_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ip_mreq {
    pub imr_multiaddr: in_addr,
    pub imr_interface: in_addr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ipv6_mreq {
    pub ipv6mr_multiaddr: in6_addr,
    pub ipv6mr_interface: c_uint,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct addrinfo {
    pub ai_flags: c_int,
    pub ai_family: c_int,
    pub ai_socktype: c_int,
    pub ai_protocol: c_int,
    pub ai_addrlen: socklen_t,
    pub ai_addr: *mut sockaddr,
    pub ai_canonname: *mut c_char,
    pub ai_next: *mut addrinfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct passwd {
    pub pw_name: *mut c_char,
    pub pw_passwd: *mut c_char,
    pub pw_uid: uid_t,
    pub pw_gid: gid_t,
    pub pw_gecos: *mut c_char,
    pub pw_dir: *mut c_char,
    pub pw_shell: *mut c_char,
}

// Aliases: a std referencia `stat64` em alguns caminhos de compatibilidade.
pub type stat64 = stat;
pub type off64_t = off_t;

pub const AT_FDCWD: c_int = -100;
pub const AT_REMOVEDIR: c_int = 0x200;
pub const AT_SYMLINK_NOFOLLOW: c_int = 0x100;
pub const AT_EACCESS: c_int = 0x200;
pub const UTIME_OMIT: c_long = 1_073_741_822;
pub const UTIME_NOW: c_long = 1_073_741_823;
pub const O_NONBLOCK: c_int = 0o4000;
pub const F_DUPFD_CLOEXEC: c_int = 1030;
pub const FIOCLEX: c_ulong = 0x5451;
pub const FIONBIO: c_ulong = 0x5421;
pub const PTHREAD_STACK_MIN: usize = 2048;
pub const SOMAXCONN: c_int = 128;
pub const TCP_NODELAY: c_int = 1;
pub const IP_TTL: c_int = 2;
pub const IP_MULTICAST_TTL: c_int = 33;
pub const IP_MULTICAST_LOOP: c_int = 34;
pub const IP_ADD_MEMBERSHIP: c_int = 35;
pub const IP_DROP_MEMBERSHIP: c_int = 36;
pub const IPV6_UNICAST_HOPS: c_int = 16;
pub const IPV6_MULTICAST_IF: c_int = 17;
pub const IPV6_MULTICAST_HOPS: c_int = 18;
pub const IPV6_MULTICAST_LOOP: c_int = 19;
pub const IPV6_ADD_MEMBERSHIP: c_int = 20;
pub const IPV6_DROP_MEMBERSHIP: c_int = 21;
pub const IPV6_V6ONLY: c_int = 26;
pub const IPPROTO_IP: c_int = 0;
pub const IPPROTO_IPV6: c_int = 41;
pub const IPPROTO_TCP: c_int = 6;
pub const SO_REUSEADDR: c_int = 2;
pub const SO_KEEPALIVE: c_int = 9;
pub const SO_BROADCAST: c_int = 6;
pub const _SC_GETPW_R_SIZE_MAX: c_int = 70;
pub const _SC_HOST_NAME_MAX: c_int = 180;
pub const SIGIO: c_int = 29;
pub const SIGPROF: c_int = 27;
pub const SIGSYS: c_int = 31;
pub const SIGTSTP: c_int = 20;
pub const SIGTTIN: c_int = 21;
pub const SIGTTOU: c_int = 22;
pub const SIGURG: c_int = 23;
pub const SIGVTALRM: c_int = 26;
pub const SIGWINCH: c_int = 28;
pub const SIGXCPU: c_int = 24;
pub const SIGXFSZ: c_int = 25;
pub const EADDRINUSE: c_int = 98;
pub const EADDRNOTAVAIL: c_int = 99;
pub const ECONNABORTED: c_int = 103;
pub const ECONNREFUSED: c_int = 111;
pub const ECONNRESET: c_int = 104;
pub const EDQUOT: c_int = 122;
pub const EFBIG: c_int = 27;
pub const EHOSTUNREACH: c_int = 113;
pub const EINPROGRESS: c_int = 115;
pub const EISCONN: c_int = 106;
pub const EMLINK: c_int = 31;
pub const EMSGSIZE: c_int = 90;
pub const ENETDOWN: c_int = 100;
pub const ENETUNREACH: c_int = 101;
pub const ENOTCONN: c_int = 107;
pub const EOPNOTSUPP: c_int = 95;
pub const ESTALE: c_int = 116;
pub const ETXTBSY: c_int = 26;

/// Macros de `wait`: no x86_64/Linux a musl as expõe como funções, mas a std
/// também espera os nomes.
pub const fn WIFEXITED(status: c_int) -> bool { (status & 0x7f) == 0 }
pub const fn WEXITSTATUS(status: c_int) -> c_int { (status & 0xff00) >> 8 }
pub const fn WIFSIGNALED(status: c_int) -> bool {
    ((status & 0x7f) + 1) as i8 >= 2
}
pub const fn WTERMSIG(status: c_int) -> c_int { status & 0x7f }
pub const fn WIFSTOPPED(status: c_int) -> bool { (status & 0xff) == 0x7f }
pub const fn WSTOPSIG(status: c_int) -> c_int { WEXITSTATUS(status) }
pub const fn WIFCONTINUED(status: c_int) -> bool { status == 0xffff }
pub const fn WCOREDUMP(status: c_int) -> bool { status & 0x80 != 0 }

pub const DT_UNKNOWN: c_uchar = 0;
pub const DT_FIFO: c_uchar = 1;
pub const DT_CHR: c_uchar = 2;
pub const DT_DIR: c_uchar = 4;
pub const DT_BLK: c_uchar = 6;
pub const DT_REG: c_uchar = 8;
pub const DT_LNK: c_uchar = 10;
pub const DT_SOCK: c_uchar = 12;
