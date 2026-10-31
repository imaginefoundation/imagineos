# Alvo Rust `x86_64-unknown-dreamcore`

Compila programas Rust direto para **DEF** (o formato que o kernel carrega),
sem passo manual de `dnu-pack`.

## Uso

```toml
# Cargo.toml do seu programa
[package]
name = "meu-app"
edition = "2024"

[workspace]                # projeto independente do workspace do kernel

[dependencies]
dreamcore-rt = { path = ".../common/userland/runtimes/dreamcore-rt" }
```

```toml
# .cargo/config.toml
[build]
target = "caminho/para/targets/x86_64-unknown-dreamcore.json"

[target.x86_64-unknown-dreamcore]
linker = "caminho/para/.build/dnulink"

[unstable]
build-std = ["core", "alloc"]
build-std-features = ["compiler-builtins-mem"]
```

```rust
#![no_std]
#![no_main]
use dreamcore_rt::{entry, print};

entry!(main);
fn main() -> i32 { print("olá\n"); 0 }
```

```sh
RUSTC_BOOTSTRAP=1 cargo +nightly build --release -Zjson-target-spec
# -> target/x86_64-unknown-dreamcore/release/meu-app  (já é DEF)
```

Ou simplesmente:

```sh
make -f build/tools.mk rust-example
```

## Como funciona

```text
src/main.rs
   │ rustc --target x86_64-unknown-dreamcore
   ▼
objetos .o (ELF)
   │ rust-lld -T <script embutido no dnulink>
   ▼
ELF executável, layout confirmado
   │ dnulink
   ▼
DEF  (DNUF + tabela de segmentos + payloads)
```

O `rustc` só fala ELF com linkers; por isso o alvo declara
`"linker": "dnulink"`. O `dnulink`:

1. repassa os argumentos ao `rust-lld`, **acrescentando** o linker script que
   separa `.text` (R-X), `.rodata` (R--) e `.data`/`.bss` (RW-);
2. lê os `PT_LOAD` resultantes;
3. escreve o contêiner DEF e valida com `imagineos_abi::def::validate_image` —
   a mesma função que o kernel usa.

### Campos-chave do target

| Campo | Valor | Motivo |
|---|---|---|
| `llvm-target` | `x86_64-unknown-none` | reusa o backend x86_64_64 bare-metal |
| `os` / `env` | `dreamcore` | `cfg(target_os = "dreamcore")` |
| `relocation-model` | `static` | sem PIC/GOT dinâmico |
| `panic-strategy` | `abort` | sem unwinding |
| `disable-redzone` | `true` | interrupções usam a pilha do kernel |
| `code-model` | `small` | o DEF só aceita VA abaixo de `0x8000_0000_0000` |
| `no-default-libraries` | `true` | sem CRT/libc do Rust |
| `entry-name` | `_start` | o loader entra em `_start` |
| `linker-flavor` | `gnu-lld` | argumentos no formato do lld |

## `dreamcore-rt`

Runtime mínimo (`common/userland/runtimes/dreamcore-rt`):

- `entry!(main)` — define `_start` e converte o retorno em `exit`;
- `#[panic_handler]` — imprime no stderr e sai com 127;
- `memcpy`/`memmove`/`memset`/`memcmp`/`bcmp`/`strlen` (exigidos pelo `core`);
- API de syscalls: `syscall`, `exit`, `write`, `print`, `eprint`, `yield_now`,
  `monotonic_microseconds`, `mmap`, `munmap`.

## Validação

`make -f build/tools.mk rust-example` produz um DEF que passa em
`validate_image`. Confirmado no binário de exemplo:

```text
magic=DNUF arch=1 type=0 fmt=1 abi=1
segments=2 entry=0x400163 base=0x400000
  seg0 va=0x400000 RX
  seg1 va=0x401000 R
```

E o código não contém nenhuma instrução `syscall` do Linux: só `int 0x80`.

## `alloc` e `Vec`/`String`/`Box`

O `dreamcore-rt` traz um `#[global_allocator]` pronto. Basta:

```rust
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;
use dreamcore_rt::{entry, install_allocator};

install_allocator!();
entry!(main);
```

O alocador abre arenas de 4 MiB com `mmap` e subaloca dentro delas. Isso é
necessário porque o kernel limita **32 faixas (VMAs) por processo**: um
`mmap` por alocação esgotaria a tabela com menos de 32 `Vec`s. Ver
`common/userland/runtimes/dreamcore-rt/src/allocator.rs`.

## `std`

A `std` do Rust compila para o alvo via `-Z build-std=std`, usando:

* a camada `os::unix::dreamcore` (patch de 117 linhas na `std` do toolchain);
* o shim `libc` sobre a musl portada;
* o `dnulink` linkando `libc.a` e um stub de unwinding.

O passo a passo para compilar um programa **seu** com `std` está em
[`howto-rust-std.md`](howto-rust-std.md). O funcionamento interno (patch da
std, shim `libc`, link) está em `std-port.md`. Exemplo pronto em
`examples/rust-std-hello`.

Atalho para criar um projeto já configurado:

```sh
std-dreamcore/new-project.sh ~/meu-app
cd ~/meu-app && ./build.sh
```

## Limitações

1. **Nightly obrigatório** para `-Z build-std` (o `core` do alvo não vem
   pré-compilado). O `make rust-example` já passa `RUSTC_BOOTSTRAP=1`.
2. **`std` exige TLS inicial no kernel**: a `std` acessa `%fs:0` e o kernel
   ainda não monta o bloco `.tdata`/`.tbss` no `exec`. Bloqueia executar
   (não compilar) programas com `std`.
3. **Sem rede**: sockets compilam e falham com `ENOSYS`.
4. **O DEF não tem `.eh_frame`**: `panic = "abort"` é a única estratégia.
