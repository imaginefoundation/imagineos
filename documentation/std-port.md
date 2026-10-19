# `std` do Rust no ImagineOS

Estado: **a `std` compila e linka para DEF** com a camada `dreamcore`, mas a
execução ainda depende de dois itens de kernel (TLS inicial da thread e
`/proc`-free `current_exe`). Este documento registra o que foi feito, como
reproduzir e o que falta.

## Como a `std` foi portada

A `std` do Rust não tem pontos de extensão: a família unix é selecionada por
`cfg(target_family = "unix")` e a camada de SO concreta por `cfg(target_os)`.
Em vez de manter um fork, o projeto aplica **um patch de 117 linhas** na `std`
distribuída pelo `rust-src`:

```sh
rustup component add rust-src --toolchain nightly
std-dreamcore/patches/apply.sh          # aplica (cria backup .orig.dreamcore)
std-dreamcore/patches/apply.sh --revert # restaura o toolchain
```

O patch faz quatro coisas pequenas:

1. `std/build.rs` — reconhece `dreamcore` como plataforma conhecida (sem ele,
   o crate exige `#![feature(restricted_std)]`).
2. `std/src/os/mod.rs` — não compila o módulo `linux` para `dreamcore`.
3. `std/src/os/unix/mod.rs` — aponta `platform` para `crate::os::unix::dreamcore`.
4. Gates pontuais: `paths` (reusa `unix`, `current_exe` por `argv[0]`),
   `random` (sem `getrandom` → fallback por relógio), `thread::set_name` e
   `args` (usa a variante `unix`).

`std/src/os/unix/dreamcore/` é novo e contém só `raw` + `fs` (subconjunto da
camada linux: sem rede, sem IPC SysV, sem `pidfd`, sem `/proc`).

## A crate `libc`

A `std` unix fala com o SO **somente** pela crate `libc`. Em
`std-dreamcore/libc-shim/` há um shim que:

* declara tipos/constantes com os valores da musl;
* declara as funções como `extern "C"`, resolvidas contra
  `third_party/musl-1.2.6/lib/libc.a` no link (a musl portada, que fala
  `int 0x80`).

O `apply.sh` adiciona `libc = { path = .../libc-shim }` ao
`[patch.crates-io]` do workspace do `rust-src` e regenera o lock — o
`build-std` resolve dependências a partir daquela árvore, não da do projeto.

## Link

O `dnulink` (linker do alvo) acrescenta automaticamente:

* o linker script (`.text` R-X, `.rodata` R--, `.data`/`.bss` RW-, `.tdata`/
  `.tbss` em `PT_TLS`);
* `std-dreamcore/unwind-stub.o` — `rust_eh_personality`/`_Unwind_*` mínimos
  (o alvo usa `panic = "abort"`);
* `third_party/musl-1.2.6/lib/libc.a`.

Caminhos alternativos: `DNULINK_MUSL`, `DNULINK_UNWIND`.

## Reproduzir

```sh
std-dreamcore/patches/apply.sh
make -f build/tools.mk dnulink
cd examples/rust-std-hello
RUSTC_BOOTSTRAP=1 cargo +nightly build --release -Zjson-target-spec -Zbuild-std=std
```

Saída esperada: `target/x86_64-unknown-dreamcore/release/rust-std-hello` em
formato DEF (validado: 3 segmentos, W^X, 14 `int 0x80`, 0 `syscall` Linux).

## TLS inicial (implementado)

A `std` acessa `%fs:0` (`mov %fs:0x0, %rax`) e as variáveis TLS com offsets
**negativos** (modelo *variant II*, o mesmo da musl). O kernel agora monta o
bloco no `exec`:

1. `dnulink` extrai o `PT_TLS` do ELF e o emite como segmento do DEF marcado
   com `DEF_SEGMENT_TLS` (bit 0 do campo `reserved` — compatível com binários
   antigos, que têm `reserved == 0`).
2. `loader::find_tls_segment` localiza esse segmento; `loader::build_tls`
   monta `[ bloco TLS: memsz ][ TCB: 0x1000 ]` **abaixo do `rsp` inicial**.
3. `fs_base = bloco + memsz`; grava o self-pointer em `%fs:0x0`.

Layout resultante (verificado contra o binário real):

```text
  rsp ──────────────► ponteiros de argv/envp
  ...
  fs_base + 0x1000 ─► fim do TCB
  fs_base + 0x0000 ─► [ self-pointer ]   (o código lê %fs:0x0)
  fs_base ──────────► fim do bloco TLS
  fs_base - memsz ──► início do bloco (.tdata, depois .tbss)
```

Conferência com o `rust-std-hello` (`PT_TLS`: `filesz=0x30`, `memsz=0x1020`):

* `%fs:0x0` carrega o self-pointer → ok;
* a variável em `bloco + 0x38` é acessada como `-0xfe8(%fs_base)` →
  `0x1020 - 0xfe8 = 0x38` → ok.

### Threads

`pthread_create` continua montando a **cópia** do bloco para a thread nova
(`__copy_tls` da musl) e o kernel só recebe o ponteiro em
`ThreadCreate.tls`. Para a musl saber o tamanho do bloco, o `_start` da `std`
consulta `SYS_TLS_INFO` e repassa a `__dreamcore_set_main_tls` — sem isso
`libc.tls_size` fica 0 e `__copy_tls` falha.

## O que ainda falta

| Item | Por quê | Onde |
|---|---|---|
| **`std::env::args`** | O loader passa `argc/argv` em registradores; testar o caminho real. | `std/src/sys/args/unix.rs` |
| **Diretórios** | Sem descritores de diretório, `read_dir` falha com `ENOTDIR`; a listagem usa `SYS_READDIR`. | kernel `abi/thread.rs` |
| **Rede** | `TcpStream`/`UdpSocket` compilam e falham com `ENOSYS`. | — |
| **`std::process`** | `Command` usa `fork` + `execve`; `execve` recusa multithread. | kernel |

## Alternativa sem patch

Se preferir não tocar no toolchain, a opção é um fork completo da `std`
(~700 arquivos) em `std-dreamcore/std/`, com o mesmo conteúdo do patch. O
custo é de manutenção a cada atualização do nightly; o patch de 117 linhas foi
escolhido justamente para minimizar isso.
