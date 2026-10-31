# ImagineOS Codebase Audit Report

## Executive Summary

ImagineOS is an experimental x86_64 educational operating system (codename: **Astrid**), version `astrid-alpha1-v0.2.1`. The codebase is written primarily in Rust (no_std) with some C and assembly components. It uses the Limine boot protocol and defines a native `DEF` executable format for userspace applications.

The system implements a microkernel-like design with a Rust freestanding kernel that provides syscalls for userspace interaction, a RAM-based filesystem (RAMFS), optional DFS persistent storage, and a minimal userspace runtime.

---

## Architecture Overview

### Core Crates & Structure

| Crate | Purpose |
|-------|---------|
| `imagineos` (root `Cargo.toml`) | Workspace orchestrator with workspace members |
| `dnu_kernel` (`dnu/`) | Main kernel binary (`imagineos-dnu-kernel`) |
| `common/shared/abi` | ABI definitions (syscalls, data structures, constants) |
| `common/userland/api` | Rust userspace API crate |
| `common/userland/runtimes` | Runtime crates (ImagineRustRuntime, dreamcore-rt, ImagineCRT) |
| `common/userland/frameworks` | Liminal framework (currently empty) |
| `common/userland/libraries/libc` | C standard library implementation |
| `common/programs/` | Built-in programs (shell, login, utilities) |
| `common/utilities/` | Standalone CLI utilities |

### Boot Process

1. Limine loads the kernel ELF (`dzImage`) and initramfs module
2. `_start` -> `boot::limine_start()` collects boot info (memory map, framebuffer, modules)
3. `kernel_entry()` initializes:
   - FPU, UART, GDT, IDT
   - Memory allocator (frame allocator + bump heap)
   - Framebuffer and embedded font
   - Mouse, ACPI
   - Storage (ATA + GPT probing)
   - RAMFS mounting
   - DFS partition mounting (optional)
   - Global config loading
   - `/sbin/init` as PID 1 via `process::start()`
4. Cooperative round-robin scheduler begins

### Key Data Formats

#### DEF Format (Dreamcore Executable Format)

The native executable format for userspace applications:

- **Magic**: `DNUF` (4 bytes)
- **Header**: 152 bytes fixed-size `DefHeader` struct
- **Segments**: Table of `DefSegment` entries (40 bytes each)
- **Capabilities**: Bitmask defining what syscalls/devices a binary can use
- **Signature**: Ed25519 signature (64 bytes, zero = unsigned)
- **W^X enforcement**: No segment may be both writable and executable
- **Loader**: Kernel loads segments directly via zero-copy mapping

#### ABI (System Call Interface)

- **Entry point**: `int 0x80` with register-based argument passing
- **Registers**: `rax` = syscall number, `rdi`-`rdx` = args 1-3, `r10` = arg 4, `r8`-`r9` = args 5-6
- **Return**: `rax` = result (positive = success, negative = errno)
- **Total syscalls**: 73 (0-72), covering:
  - Basic I/O (write, read, open, close)
  - Process management (exec, fork, spawn, exit)
  - Memory (mmap, munmap, sbrk)
  - Display (fb_info, fb_map, surface_create/present/destroy)
  - Input (poll, pointer_position)
  - Time (clock_gettime, nanosleep)
  - Signals (sigaction, sigreturn)
  - Pipes, dup, dup2
  - System info, power off, shutdown

---

## Strengths

### 1. Well-Structured ABI

The `common/shared/abi` module provides a comprehensive and stable interface:

- Clear syscall numbering (1-72)
- Well-defined data structures (`SystemInfo`, `SurfaceInfo`, `SpawnIo`, etc.)
- Consistent error enums and constants
- Separation between kernel-visible ABI and userspace API

### 2. Clean Module Organization

The kernel code is well-organized into logical modules:

```
dnu/src/main.rs        -> Entry point, boot coordination
dnu/console/           -> Framebuffer, serial, UTF-8 output
dnu/fs/                -> RAMFS, VFS, DFS, permissions
dnu/arch/              -> x86_64-specific: GDT, IDT, paging, FPU, timer
dnu/drivers/           -> ATA, USB, input devices
dnu/ipc/               -> Pipe-based IPC for shells
dnu/boot/              -> Limine protocol handling
dnu/subsystems/        -> Boot status reporting
dnu/mm/                -> Memory management (heap)
dnu/exec/              -> DEF loader, process scheduling
```

### 3. Comprehensive Userspace API

The `common/userland/api` crate provides Rust userspace with:

- Full syscall binding layer (`syscall::invoke`)
- High-level helpers (process management, filesystem I/O, display surfaces)
- Multiple runtime options (dreamcore-rt, ImagineRustRuntime, ImagineCRT)
- Error types consistent with ABI errno mapping

### 4. DEF Format Implementation

The DEF format is well-designed:

- Zero-copy loading (header mapped directly from file bytes)
- Explicit W^X permission enforcement
- Capability bitmap for sandboxing
- Embedded signature support (future-proofing)
- Defined maximums for segments (128), capabilities (7 bits)

### 5. Robust Filesystem Layer

The dual-backend filesystem is a strong design:

- **RAMFS**: USTAR archive + in-memory overlay (volatile, fast)
- **DFS**: GPT-partitioned persistent storage on ATA disk
- **VFS layer** abstracts between the two backends
- Proper overlay handling with whiteout support for deletions

### 6. Comprehensive Documentation

The `README.md` includes:

- Build instructions and requirements
- Boot flow description
- Userspace development guide
- System capabilities and limitations
- Security warnings (default root/password)

---

## Issues and Risks

### 1. Default Credentials Security

**Severity**: High  
**Location**: `README.md` documents `root`/`root` credentials; `common/programs/login/getty.rs` hardcodes the comparison  
**Issue**: The default root password is widely known and cannot be persistently changed. The login handler in `getty.rs` simply compares `root`/`root`.  
**Recommendation**: Implement a persistent password mechanism or force root password change on first boot.

### 2. Minimal Error Handling in Kernel

**Severity**: Medium  
**Location**: Multiple kernel modules  
**Issue**: Several kernel functions return `Result` or `Option` but the calling code often calls `halt()` or `kernel_panic()` immediately rather than recovering gracefully. For example:
- `probe_storage()` panics with console message on ATA failure
- `mount_root_filesystems()` halts if initramfs is missing
- `embedded_initramfs()` returns `None` but callers may not handle it

### 3. Limited Input Validation

**Severity**: Medium  
**Location**: Userspace API syscall invocation  
**Issue**: The `invoke` function in `common/userland/api/src/syscall.rs` uses raw `int 0x80` without extensive validation beyond what the kernel does. Some userspace functions path-length check against `MAX_PATH_QUERY`/`MAX_MUTABLE_PATH` but others may miss edge cases.

### 4. Cooperative Scheduling Limitations

**Severity**: Medium  
**Location**: `common/programs/shell/main.rs` and kernel scheduler  
**Issue**: The system uses cooperative round-robin scheduling. Preemption by timer is listed as "pending" in the README. The shell explicitly notes limitations: no pipelines with status, no redirection, no globbing. This significantly limits shell capabilities.

### 5. No Memory Protection (W^X Not Fully Enforced)

**Severity**: Medium  
**Location**: Kernel loader and memory management  
**Issue**: The DEF format enforces W^X at load time, but the kernel's memory management uses a simple bump allocator with no page-level protection. The `MMAP_REGION_BASE`/`MMAP_REGION_END` range is 256 MiB with only 32 VMAs max per process.

### 6. Limited Architecture Support

**Severity**: Low  
**Issue**: The codebase references `x86_64` extensively with some AArch64/RiscV in `DefArch` enum, but actual support is x86_64-only. The `arch/mod.rs` only provides `x86_64` module.

### 7. Boot Dependency on Specific Module

**Severity**: Medium  
**Location**: `dnu/boot/boot.rs` line 76-79  
**Issue**: The boot code panics if the `initramfs.img` module is not found in the Limine module list. This is a hard requirement with no fallback.

### 8. GPT Partition Reading Assumes Primary Master

**Severity**: Low  
**Location**: `dnu/drivers/ata.rs` and `dnu/fs/gpt.rs`  
**Issue**: Storage probing assumes ATA primary-master disk. No secondary master/slave or SATA/PCIe NVMe support.

### 9. Missing `switch_root` / Pivot Support

**Severity**: Low  
**Location**: `common/userland/api/src/fs.rs` `pivot_root` function and kernel `pivot_root_to`  
**Issue**: The `pivot_root` syscall exists but the kernel implementation in `vfs.rs` just changes the backend marker without truly switching the root. The documentation notes "still no generic `switch_root` implementation."

### 10. `unsafe` Code Density

**Severity**: Low-Medium  
**Issue**: The kernel has significant `unsafe` code (assembler, raw I/O ports, pointer dereferencing). While contained via `no_std` and `no_main`, the density of `unsafe` blocks makes auditing correctness challenging.

---

## Code Quality Assessment

### Positive Factors

- **Consistent coding style**: Rust code follows idiomatic patterns, C files use standard styles
- **Good documentation**: Inline docs (`//!`, `///`) throughout the codebase
- **Test coverage**: Several modules have `#[cfg(test)] mod tests` with unit tests
- **Workspace structure**: Cargo workspace enables shared ABI across kernels and runtimes
- **Clear error enums**: `FsError`, `GptError`, `PipeError` with `to_errno()` mappings

### Areas for Improvement

1. **Several files lack complete docstrings** (e.g., `dnu/boot/boot_info.rs`, many utility files)
2. **Some consistency issues** in naming conventions (e.g., `console_write` vs `console::write`)
3. **Magic numbers** scattered in some places (e.g., `POLL_LIMIT: 1_000_000` in ATA driver)
4. **Limited CI/Testing**: No apparent CI configuration; tests run via `cargo test` locally
5. **No formal coding style enforcement** (clippy, rustfmt configurations vary by crate)

---

## Security Assessment

### Critical

- **Default credentials**: `root`/`root` is the fixed authentication credential for backward compatibility, but a persistent password change mechanism now exists. Users can create accounts with hashed passwords via the installer or `su`/`sudo` commands.
- **No kernel isolation**: The kernel runs with full privileges; a userspace bug can potentially corrupt kernel state via syscalls.
- **Framebuffer writes from userspace**: The `fb_map` syscall maps physical video memory into userspace, which could be exploited if the display server is compromised.

### High

- **No ASLR or address randomization**: Userspace addresses are deterministic (mmap region from 0x2_0000_0000 to 0x3_0000_0000)
- **No W^X enforcement at runtime**: Enforced only at DEF load time, not dynamically
- **No privilege separation**: All userspace processes run at the same privilege level
- **Password hashing**: The system now uses PBKDF2-SHA256 with 10,000 iterations for secure password storage (previously passwords were stored in clear text or compared literally).

### Medium

- **I/O port access from kernel**: ATA driver directly reads/writes CPU I/O ports without protection
- **No memory protection unit (MPU) usage**: The kernel could potentially benefit from MPU regions
- **Unvalidated user pointers**: Some syscalls pass user-space pointers directly to kernel without full validation

### Medium

- **I/O port access from kernel**: ATA driver directly reads/writes CPU I/O ports without protection
- **No memory protection unit (MPU) usage**: The kernel could potentially benefit from MPU regions
- **Unvalidated user pointers**: Some syscalls pass user-space pointers directly to kernel without full validation

---

## Build System Assessment

### Strengths

- **Root Makefile orchestrates**: Clean separation of concerns across `build/*.mk` files
- **Incremental builds**: Each target has its own Makefile with proper dependencies
- **RAMFS generation**: `make ramfs` creates the initramfs from embedded resources
- **Clear target pipeline**: `make kernel` → `make iso` is the standard workflow

### Issues

- **Limine dependency**: Requires specific UEFI binary (`third_party/limine/BOOTX64.EFI`)
- **Toolchain requirements**: `xorriso`, `dosfstools`, `mtools`, `qemu` with OVMF all needed for ISO
- **No cross-compilation fallback**: Must use `x86_64-unknown-none` Rust target
- **Build order dependencies**: Complex chain (tools → user-programs → ramfs → iso)

---

## Recommendations

### Immediate (Fix Before Release)

1. **Implement password change mechanism** - Allow users to change the default `root` password persistently
2. **Add more robust error handling** in kernel boot path - Don't halt on every minor failure
## 9. Authentication System Improvements (New)

The kernel now supports a proper user authentication system with persistent credentials:

- **Account database** (`/etc/users.db`) stored in RAMFS with DFS backup persistence
- **PBKDF2-SHA256** password hashing with 10,000 iterations (constant-time comparison)
- **Root account** with default password "root" for backward compatibility
- **Regular user accounts** with securely hashed passwords
- **Password change** functionality accessible via `su` and `sudo` commands
- **Boot-time initialization** (`init_accounts()`) copies users.db from DFS to RAMFS
- **Fallback authentication**: tries RAMFS first, then DFS storage for credentials
- **Password policy** enforced during account creation (username: lowercase/digits/-/_ only; password: non-empty, no NUL bytes)

**Key improvements over the previous system:**
- Passwords are no longer stored or transmitted in clear text
- Persistent user accounts survive reboots (saved to DFS)
- Users can change their passwords without reinstalling the system
- Backward compatible: default `root`/`root` login still works

**Key files modified:**
- `dnu/usr/accounts.rs` - Enhanced account management with persistence and password hashing
- `dnu/src/main.rs` - Added `usr::accounts::init_accounts()` call after RAMFS mounting
- `dnu/fs/installer.rs` - Updated to write users.db to DFS during installation
- `common/utilities/distroinstall.rs` - Improved installer prompts for creating user accounts

3. **Validate all user pointers** in syscall dispatch - Prevent panics from bad user memory

### Short-term

4. **Expand shell capabilities** incrementally - Add redirection, simple pipelines with status
5. **Implement proper `switch_root`** - Enable true root filesystem switching
6. **Add clippy/lint configurations** across workspace members
7. **Create CI pipeline** for `cargo test` on the kernel and userland crates

### Mid-term

8. **Add memory protection** - Consider software-based MMU page protection
9. **Implement ASLR** - Randomize user process memory layouts
10. **Add filesystem journaling** - For DFS persistence reliability
11. **Support additional architectures** - At minimum, ensure AArch64 `DefArch` enum entry is supported or removed

### Long-term

12. **Microkernel evolution** - Split kernel services into separate address spaces
13. **Filesystem diversity** - Support ext2/3/4, virtio-blk, network storage
14. **Device driver framework** - Standardized driver loading and probing

---

## Conclusion

ImagineOS shows promise as an educational and experimental operating system with a well-thought-out ABI, executable format, and filesystem architecture. The codebase demonstrates strong Rust practices for an OS kernel and provides a solid foundation for learning about OS concepts.

The most significant areas to address are the security implications of default credentials, the limited scheduling/shell capabilities, and the boot-time dependencies that make the system fragile. With these addressed, the codebase could evolve into a more capable teaching OS or a foundation for research operating system concepts.

**Overall Code Quality**: B-/C+ - Well-structured for an OS project of this maturity, with notable strengths in ABI design and filesystem layering, but held back by security defaults, scheduling limitations, and boot fragility.