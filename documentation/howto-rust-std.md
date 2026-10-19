# Como compilar um programa Rust com `std` para o ImagineOS

Este guia leva um projeto Rust novo (`cargo new`) até um binário **DEF** que o
kernel carrega. Foi verificado de ponta a ponta com um projeto fora do
repositório.

## Resumo em três comandos

```sh
# 1. Primeira vez apenas: aplica a camada `dreamcore` na std do nightly.
std-dreamcore/patches/apply.sh

# 2. Constrói o linker do alvo (`.build/dnulink`).
make -f build/tools.mk dnulink

# 3. No seu projeto:
RUSTC_BOOTSTRAP=1 cargo +nightly build --release -Zjson-target-spec
```

A saída é `target/x86_64-unknown-dreamcore/release/<seu-crate>` — **já é DEF**
(`DNUF` nos primeiros bytes), sem passo de `dnu-pack`.

---

## Pré-requisitos

| Item | Como obter |
|---|---|
| Toolchain **nightly** | `rustup toolchain install nightly` |
| Código-fonte da std | `rustup component add rust-src --toolchain nightly` |
| `make`, `gcc`, `patch`, `python3` | do sistema |
| Musl portada | já em `third_party/musl-1.2.6`, construída com `make musl` |

`RUSTC_BOOTSTRAP=1` libera `-Z build-std` na toolchain **stable** do ponto de
vista de estabilidade; é o que permite recompilar `core`/`std` para o alvo. Se
usar `cargo +nightly` diretamente, a flag continua necessária apenas por causa
de `-Zjson-target-spec`.

---

## Passo 1 — Aplicar a camada `dreamcore` na `std`

A `std` do Rust escolhe a camada de sistema operacional por `target_os`. O
projeto não mantém um fork: aplica **um patch pequeno** na std distribuída pelo
`rust-src`.

```sh
std-dreamcore/patches/apply.sh
```

Saída esperada:

```
patching file std/src/sys/pal/unix/mod.rs
camada dreamcore aplicada em: ~/.rustup/.../library/std
```

O script:

1. faz backup da std original em `std.orig.dreamcore` (uma vez);
2. aplica `patches/std-dreamcore.patch` (reconhece `dreamcore` como
   plataforma, aponta `platform` para a camada própria, ajusta paths/random/
   thread/args e instala o hook de TLS);
3. copia `patches/os-unix-dreamcore/` para `std/src/os/unix/dreamcore/`;
4. adiciona o shim `libc` ao `[patch.crates-io]` do workspace do `rust-src` e
   regenera o `Cargo.lock` — é **assim** que o `build-std` passa a usar o shim
   em vez da crate `libc` do registry.

Para desfazer:

```sh
std-dreamcore/patches/apply.sh --revert
```

> **Reaplique após atualizar o nightly.** Uma atualização de toolchain repõe o
> `rust-src` original. O script detecta e reaplica; versões muito novas do
> `rustc` podem exigir ajuste no patch.

### Verificar

```sh
SYSRC=$(rustc +nightly --print sysroot)/lib/rustlib/src/rust/library
grep -l dreamcore $SYSRC/std/build.rs $SYSRC/std/src/os/unix/mod.rs \
    $SYSRC/std/src/sys/pal/unix/mod.rs
ls $SYSRC/std/src/os/unix/dreamcore/     # fs.rs mod.rs raw.rs
```

## Passo 2 — Construir o linker do alvo

```sh
make -f build/tools.mk dnulink
```

Isso produz `.build/dnulink`. Ele é o `linker` declarado em
`targets/x86_64-unknown-dreamcore.json`; chama o `rust-lld`, aplica o linker
script, linka a musl (`third_party/musl-1.2.6/lib/libc.a`) e o stub de
unwinding, e converte o ELF para DEF.

Se a musl ainda não foi construída:

```sh
make musl
```

## Passo 3 — Configurar o projeto

`Cargo.toml`:

```toml
[package]
name = "meuapp"
version = "0.1.0"
edition = "2021"          # 2024 também funciona

# Projeto independente: o alvo é próprio e o build usa -Z build-std.
[workspace]

[profile.release]
panic = "abort"           # o alvo não tem libunwind
opt-level = "s"
lto = true
codegen-units = 1
```

`.cargo/config.toml` (troque `RAIZ` pelo caminho do repositório do ImagineOS):

```toml
[build]
target = "RAIZ/targets/x86_64-unknown-dreamcore.json"

[target.x86_64-unknown-dreamcore]
linker = "RAIZ/.build/dnulink"

[unstable]
build-std = ["std"]
build-std-features = ["compiler-builtins-mem"]

# NÃO declare `[patch.crates-io] libc` aqui: o build-std resolve as dependências
# da std pela árvore do rust-src, onde o `apply.sh` já instalou o shim.
# Um patch no projeto gera o aviso "was not used in the crate graph".
```

`src/main.rs` — Rust normal, com `fn main`:

```rust
use std::collections::HashMap;

fn main() {
    let mut counts = HashMap::new();
    for word in "o rato roeu a roupa do rei de roma".split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    let mut pairs: Vec<_> = counts.into_iter().collect();
    pairs.sort();
    println!("{:?}", pairs);
}
```

> **Não** use `#![no_std]`/`#![no_main]` aqui: a `std` fornece o `_start`.

## Passo 4 — Compilar

```sh
RUSTC_BOOTSTRAP=1 cargo +nightly build --release -Zjson-target-spec
```

`-Zjson-target-spec` é obrigatório: o alvo é um arquivo `.json`. Como já está
em `[build] target`, não é preciso repetir `--target`.

Saída:

```
target/x86_64-unknown-dreamcore/release/meuapp
```

Confira que é DEF:

```sh
head -c4 target/x86_64-unknown-dreamcore/release/meuapp   # DNUF
```

## Passo 5 — Instalar na imagem e executar

Copie o binário para a árvore do ramfs antes de gerar a ISO:

```sh
cp target/x86_64-unknown-dreamcore/release/meuapp build/ramfs/bin/
make iso
```

No shell do ImagineOS:

```
/b $ meuapp
```

(Os exemplos `rust-hello` e `rust-std-hello` são copiados automaticamente por
`build/ramfs.mk` quando existem.)

---

## Diferenças em relação ao Rust "normal"

| Assunto | Comportamento no ImagineOS |
|---|---|
| `std::net` | Compila; toda operação falha com `ENOSYS` (não há pilha TCP/IP). |
| `std::fs` | Funciona para arquivos; `read_dir` falha sem descritores de diretório. |
| `std::thread` | Funciona (`pthread_create` → `SYS_THREAD_CREATE` + TLS por thread). |
| `std::process::Command` | Limitado: `execve` recusa processo multithread. |
| `panic` | `abort` (não há unwinding); mensagem no stderr e código 127. |
| Alocador | Arenas de 4 MiB por `mmap` (limite de 32 VMAs por processo). |
| TLS | Montado pelo kernel no `exec` a partir do `PT_TLS` do DEF. |

## Solução de problemas

| Sintoma | Causa e solução |
|---|---|
| `error[E0658]: #![feature] may not be used on stable` | Falta `RUSTC_BOOTSTRAP=1` ou usar `cargo +nightly`. |
| `error: failed to run custom build command ... build-script-build` / std sem `restricted_std` | A camada não foi aplicada: rode `std-dreamcore/patches/apply.sh`. |
| `cannot find function \`xyz\` in crate \`libc\`` (mesmo com `xyz` no shim) | **Cache obsoleto do `libc`** no `build-std`. Apague `target/` do projeto e recompile. O `build.sh` gerado por `new-project.sh` detecta isto sozinho (hash do shim) e limpa o cache. |
| `warning: patch \`libc ...\` was not used in the crate graph` | Esperado se você declarar `[patch.crates-io] libc` no `.cargo/config.toml` do **projeto**. O `build-std` resolve a std pelo `rust-src`, onde o patch já está aplicado. Remova a seção do projeto: ela não tem efeito. |
| `unresolved import libc::...` | A camada não foi aplicada ao sysroot: rode `std-dreamcore/patches/apply.sh` e confira `grep std-dreamcore $SYSRC/Cargo.toml`. |
| `rust-lld: error: unable to find library -lc` ou símbolos indefinidos | A musl não foi construída: `make musl`. |
| `dnulink: não consegui executar rust-lld` | Rode com `cargo +nightly`, ou aponte `DNULINK_LLD=/caminho/rust-lld`. |
| Binário sem TLS (3 segmentos em vez de 4) | A `std` veio do cache antigo: apague `target/` e recompile. |
| `#PF` imediato em `%fs` | O kernel da imagem é antigo: recompile (`.build/dzImage`) antes de `make iso`. |

## Variáveis de ambiente úteis

| Variável | Efeito |
|---|---|
| `DNULINK_MUSL` | Caminho alternativo para `libc.a` da musl. |
| `DNULINK_UNWIND` | Caminho alternativo para o stub de unwinding. |
| `DNULINK_LLD` | Caminho do `rust-lld` usado pelo `dnulink`. |
| `DNULINK_KEEP_ELF` | Preserva o ELF intermediário para inspeção (`readelf -l`). |
