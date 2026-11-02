# AGENTS.md — Guia de Desenvolvimento do ImagineOS

Este arquivo contém instruções para desenvolvedores (humanos e agentes de IA) que trabalham no ImagineOS. Ele cobre a estrutura do repositório, como construir e testar, convenções de código, arquitetura do kernel e como adicionar novos recursos.

---

## 1. Visão Geral

O **ImagineOS** é um sistema operacional x86_64 experimental e educacional, de codinome **Astrid**. Seu kernel, chamado **DNU** (Dreamcore Kernel), é escrito em Rust `no_std` e usa o protocolo de boot do Limine.

### Separação de Responsabilidades

| Componente | Nome | Localização | Responsabilidade |
|-----------|------|-------------|------------------|
| **Kernel** | DNU / Dreamcore | `dnu/` | Gerenciamento de memória, drivers, syscalls, execução de processos, framebuffer |
| **Usuáriospace** | ImagineOS | `imagine-base/`, `imagine-common/`, `imagine-tools/` | Programas, shell, libc, runtimes, ferramentas de build |

**Regra de ouro:** O kernel DNU **não** deve conter código de interface gráfica, janelas, compositor ou instalador de disco. Esses componentes pertencem ao userspace (`imagine-base/gui/`, `imagine-common/programs/`).

---

## 2. Estrutura do Repositório

```
imagineOS/
├── build.py                     # Script principal de build (delega para imagine-tools/build/)
├── Cargo.toml                   # Workspace Rust do userspace
├── dnu/                         # Submódulo: kernel DNU
│   ├── Cargo.toml
│   ├── Makefile                 # Build do kernel e ferramentas de host
│   ├── src/
│   │   ├── main.rs              # Ponto de entrada do kernel (kernel_entry)
│   │   ├── allocator.rs         # GlobalAlloc (linked list)
│   │   ├── boot.rs              # Protocolo Limine
│   │   ├── memory.rs            # Alocador de frames físicos
│   │   ├── paging.rs            # Paginação de memória
│   │   ├── gdt.rs, idt.rs, fpu.rs
│   │   ├── memory.rs
│   │   ├── framebuffer.rs       # Framebuffer do Limine
│   │   ├── gpt.rs, dfs.rs, ramfs.rs, vfs.rs
│   │   ├── exec_def.rs, exec_loader.rs, exec_process.rs, exec_space.rs
│   │   ├── abi/, abi_user.rs, abi_dispatch.rs, ... (syscalls)
│   │   ├── keyboard.rs, mouse.rs, ata.rs, block.rs, acpi.rs, time.rs
│   │   ├── subsystems.rs        # Relatório de estado de subsistemas
│   │   ├── allocator.rs         # Alocador de heap (linked list)
│   │   ├── dzimage.rs           # Leitura/validação de dzImage
│   │   ├── config.rs            # Configuração global (keyboard layout)
│   │   ├── distro.rs            # Leitura de nome da distribuição
│   │   └── drivers_hub.rs       # Hub de drivers (block, input, video, platform)
│   ├── target/                  # Build do kernel (x86_64-unknown-none)
│   ├── .build/                  # Artefatos de build (dzImage, dnu-pack, etc.)
│   └── linker.ld                # Linker script do kernel
├── imagine-base/                # Código compartilhado do ImagineOS
│   ├── abi/                     # Contrato binário (DEF, syscalls, estruturas)
│   ├── userland/
│   │   ├── api/                 # API do userspace (syscalls, structs)
│   │   ├── runtimes/            # Runtimes Rust e C
│   │   └── linker.ld            # Linker script de userspace
│   ├── libc/                    # Libc estática (libdreamcore.a)
│   ├── shell/                   # Shell do ImagineOS
│   ├── gui/                     # Interface gráfica (GDI, Window Manager, Compositor)
│   │   ├── gdi.rs
│   │   ├── window.rs
│   │   └── compositor.rs
│   ├── config/
│   │   └── boot/
│   │       ├── BOOTX64.EFI      # Bootloader Limine UEFI
│   │       └── limine.conf      # Configuração de boot
│   ├── ramfs/                   # Esqueleto do sistema de arquivos volátil
│   ├── std-dreamcore/           # Porta do std do Rust
│   └── README.md
├── imagine-common/              # Código compartilhado adicional
│   ├── programs/
│   │   ├── login/               # getty (login)
│   │   └── sysman/              # init (PID 1)
│   ├── shell/                   # Shell do ImagineOS
│   ├── systemfetch/             # Utilitário systemfetch
│   ├── utilities/               # Utilitários C (cat, ls, etc.)
│   ├── userland/                # API e runtimes de userspace
│   └── documentation/           # Documentação técnica
├── imagine-tools/               # Ferramentas de build e host
│   ├── build/                   # Scripts de build (build.py, kernel.py, iso.py, etc.)
│   ├── isobuilder/              # Construtor de ISO UEFI
│   ├── programs/                # Programas de userspace (shell, liminal, etc.)
│   ├── utilities/               # Utilitários C (hello_c, etc.)
│   └── README.md
└── DONOTCOMMIT/                 # NÃO MODIFICAR (submódulos binários, etc.)
```

---

## 3. Como Construir e Testar

### Pré-requisitos

- Rust nightly + stable com alvo `x86_64-unknown-none`
- Python 3
- `xorriso`, `dosfstools`, `mtools`, `qemu-system-x86_64`, `OVMF`

### Comandos de Build

```bash
# Inicializar submódulos
git submodule update --init --recursive

# Instalar toolchain Rust
rustup toolchain install nightly
rustup target add x86_64-unknown-none

# Build completo (gera ISO)
python3 build.py iso

# Build apenas o kernel
python3 build.py kernel

# Build apenas userland
python3 build.py userland

# Montar apenas a árvore do RAMFS
python3 build.py ramfs

# Compilar ferramentas de host (dnu-pack, dnulink, dfs-image, dzimage)
python3 build.py tools

# Gerar e rodar no QEMU
python3 build.py run

# Limpar artefatos
python3 build.py clean
```

### Resultado Esperado

- ISO: `.build/distro/dreamcore-YYYY-MM-DD-HH-MM-astrid.iso`
- Kernel ELF: `.build/iso/kernel.elf`
- dzImage: `.build/dzImage`

### Testar no QEMU

```bash
qemu-system-x86_64 -machine q35 -m 512M -serial stdio -vga std \
  -drive if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -cdrom .build/distro/dreamcore-*.iso
```

---

## 4. Convenções de Código

### Rust (Kernel DNU)

- **Edition 2024**, `#![no_std]`, `#![no_main]`
- Use `#![allow(unsafe_op_in_unsafe_fn)]` no topo dos arquivos do kernel
- Funções `unsafe fn` usam operações `unsafe` no corpo por contrato (asm de E/S, voláteis de MMIO)
- Módulos são declarados em `main.rs` com `#[path = "arquivo.rs"]` quando o nome do arquivo difere do nome do módulo
- Módulos-hub (`usr_hub.rs`, `arch_hub.rs`, `exec_hub.rs`, `init_hub.rs`, `console_hub.rs`, `drivers_hub.rs`) declaram suas folhas
- Não use `unsafe` desnecessariamente em blocos; mantenha o estilo de contrato-por-função

### Estrutura de um Módulo do Kernel

```rust
//! Descrição do módulo.
//! Mais detalhes sobre o que ele faz.

use core::...;

// Estruturas e implementações

pub struct MinhaEstrutura {
    campo: u32,
}

impl MinhaEstrutura {
    pub fn novo() -> Self {
        Self { campo: 0 }
    }
    
    pub unsafe fn operacao(&mut self) {
        // código unsafe aqui
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn testa_algo() {
        // teste aqui
    }
}
```

### Nomenclatura

- Structs: `PascalCase`
- Funções/métodos: `snake_case`
- Constantes: `SCREAMING_SNAKE_CASE`
- Módulos: `snake_case`
- Traits: `PascalCase`

### Erros Comuns a Evitar

1. **Não modifique o `DONOTCOMMIT/`** — contém arquivos binários de terceiros
2. **Não coloque código de GUI no kernel** — GDI, Window Manager e Compositor pertencem a `imagine-base/gui/`
3. **Não use `unsafe` desnecessariamente** — prefira código seguro quando possível
4. **Não modifique o linker script sem entender as implicações** — endereços virtuais/físicos críticos
5. **Sempre teste no QEMU antes de commitar**

---

## 5. Arquitetura do Kernel DNU

### Fluxo de Boot

1. `_start()` → `boot::limine_start()`
2. `limine_start()` coleta respostas do Limine (memory map, framebuffer, módulos, HHDM, etc.)
3. `kernel_entry()` é chamado com ponteiro para `BootInfo`
4. Inicialização em ordem:
   - FPU, GDT, IDT
   - Memória (alocador de frames)
   - Alocador global (GlobalAlloc)
   - Framebuffer, drivers de vídeo/teclado/mouse
   - ACPI, platform
   - Sistema de arquivos (RAMFS, DFS opcional)
   - Configuração global
   - Loop infinito (GUI roda em loop com `hlt`)

### Alocação de Memória

- **Frames físicos:** `memory::init()` + `memory::allocate_frame()`
- **Heap do kernel:** `allocator::allocator_init()` (linked list allocator, 1 MiB estático)
- **Usuáriospace:** `Layout` + `GlobalAlloc` implementado no allocator do kernel

### Programas de Usuáriospace (DEF)

- Formato: `dnu-pack` converte ELF → DEF (Dreamcore Executable Format)
- Carregados pelo kernel via `exec_def.rs` e `exec_loader.rs`
- Instalados no RAMFS em `/bin/`, `/sbin/`, `/bin/utilities/`

---

## 6. Como Adicionar Novos Recursos

### Adicionar um Novo Driver de Kernel

1. Crie o arquivo em `dnu/src/drv_nome.rs`
2. Implemente o trait `BlockDevice` (para armazenamento) ou implemente diretamente
3. Declare o módulo em `drivers_hub.rs`
4. Registre no `drivers::platform::register()` ou chame diretamente em `main.rs`

### Adicionar uma Nova Syscall

1. Adicione a constante `SYS_*` em `imagine-base/abi/src/lib.rs`
2. Implemente o handler em um arquivo `abi_*.rs` (ex: `abi_fs.rs`, `abi_process.rs`)
3. Registre o dispatch em `abi_dispatch.rs`
4. Atualize a documentação da ABI se necessário

### Adicionar um Programa de Usuáriospace

1. Crie o fonte em `imagine-common/programs/` ou `imagine-tools/programs/`
2. Crie um `Cargo.toml` com `edition = "2024"` e dependências necessárias
3. Adicione ao `userland.py` ou `components.py` no `imagine-tools/build/`
4. O programa será compilado como ELF e empacotado em DEF via `dnu-pack`

### Adicionar uma Syscall de Sistema

1. Adicione a constante em `imagine-base/abi/src/lib.rs`
2. Implemente o handler em `dnu/src/abi_*.rs`
3. Adicione o dispatch em `dnu/src/abi_dispatch.rs`
4. Teste com um programa de userspace

---

## 7. Testes

### Testes do Kernel

Os testes do kernel estão em `dnu/tests/` e rodam no host (não no kernel bare-metal):

```bash
cd dnu
make test
```

### Testes de Integração (QEMU)

```bash
python3 build.py iso
python3 build.py run
```

Verifique:
- Boot chega até "DNU Astrid w/ Dreamcore Kernel"
- Frame allocator mostra frames disponíveis
- Timer funciona (timestamps nos logs)
- Framebuffer funciona (se houver saída de vídeo)
- Sem panic do kernel

---

## 8. Problemas Comuns e Soluções

| Problema | Causa Provável | Solução |
|----------|---------------|---------|
| `error[E0432]: cannot find module` | Caminho incorreto no `mod` | Verifique `#[path = "..."]` em `main.rs` |
| `error[E0433]: cannot find type` | Módulo não declarado | Adicione `mod nome;` em `main.rs` ou hub correspondente |
| `error[E0308]: mismatched types` | Tipos incompatíveis | Verifique tipos de ponteiro (`*const u8` vs `*const i8`) |
| `error[E0425]: cannot find value` | Módulo não declarado | Adicione o módulo em `main.rs` ou hub |
| Build falha em `make tools` | `abi-bundle.sh` ausente | Crie o arquivo em `dnu/tools/abi-bundle.sh` |
| Build falha em `dzimage.rs` | Caminho do arquivo incorreto | Verifique `$(ROOT)/src/dzimage.rs` no Makefile |
| QEMU não boota | ISO não gerada corretamente | Rode `python3 build.py clean && python3 build.py iso` |

---

## 9. Regras de Ouro

1. **Não modifique `DONOTCOMMIT/`** — contém arquivos binários de terceiros
2. **Kernel = DNU, Sistema = ImagineOS** — não misture nomes nos códigos
3. **GUI fica no userspace** — `imagine-base/gui/`, não no kernel
4. **Sempre teste no QEMU antes de commitar**
5. **Mantenha o `no_std` no kernel** — não use `std` no kernel
6. **Documente novas syscalls** em `imagine-common/documentation/`
7. **Mantenha o linker script consistente** — endereços virtuais/físicos são críticos

---

## 8. Contato e Recursos

- Repositório principal: `imagineos/`
- Kernel DNU: `dnu/`
- Documentação: `imagine-common/documentation/`
- Issues e discussões: via GitHub (repositório do projeto)

---

Última atualização: Novembro 2026