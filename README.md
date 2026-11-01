# Imagine Operating System (astrid-alpha1-v0.2.1)

O ImagineOS é um sistema operacional x86_64 experimental e educacional, de codinome **Astrid**. O kernel é Rust freestanding e usa o protocolo de boot do Limine. Este é um estágio inicial de bring-up, ainda não é um sistema operacional de uso geral.

## Estrutura do Repositório

O código está organizado em três submódulos principais:

| Submódulo | Conteúdo |
|-----------|----------|
| `dnu/` | Kernel (Rust freestanding), formato DEF, ferramentas de host (dnu-pack, dnulink, dfs-image, dzimage), sistema de arquivos DFS |
| `imagine-common/` | Código compartilhado: ABI (`abi/`), API do userspace (`userland/api/`), libc (`libc/`), runtimes (`userland/runtimes/`), linker script (`linker.ld`), programas compartilhados (`programs/login/`, `programs/sysman/`), esqueleto do RAMFS (`ramfs/`), documentação (`documentation/`), porta do std (`std-dreamcore/`) |
| `imagine-tools/` | Programas específicos e ferramentas: shell (`programs/shell/`), Liminal server/client (`programs/liminalServer/`, `programs/liminalClient/`), utilitários (`utilities/`), scripts de build (`build/`), isobuilder (`isobuilder/`), systemfetch (`systemfetch/`) |

## Compilação

Requisitos: Python 3, Rust **nightly** e **stable** com o alvo
`x86_64-unknown-none`, os submódulos inicializados, `xorriso`, `dosfstools`,
`mtools` e QEMU com OVMF para testar o boot.

```sh
rustup toolchain install nightly
rustup target add x86_64-unknown-none
git submodule update --init --recursive

python3 build.py iso       # pipeline completo: userland -> ramfs -> dzImage -> ISO
python3 build.py run       # compila a ISO e inicia no QEMU/OVMF
```

O pipeline completo está no script `build.py` da raiz (delega para
`imagine-tools/build/build.py`). Cada etapa roda isolada também:

| Comando | Responsabilidade |
|---------|------------------|
| `python3 build.py userland` | apps, utilitários Rust/C e libc em `.build/user` |
| `python3 build.py components` | componentes dos submódulos (shell, Liminal, desktop) |
| `python3 build.py ramfs` | monta a árvore do ramfs em `.build/ramfs` |
| `python3 build.py kernel` | kernel e dzImage (via submódulo DNU) |
| `python3 build.py iso` | tudo acima + ISO inicializável (via `imagine-tools/isobuilder`) |
| `python3 build.py clean` | remove artefatos de build |
| `python3 build.py tools` | compila ferramentas de host (dnu-pack, dnulink, dfs-image) |

A ISO somente UEFI é gravada em
`.build/distro/dreamcore-YYYY-MM-DD-HH-MM-astrid.iso`.

## Componentes e submódulos

Os programas compilados para o ImagineOS vivem em submódulos e pastas locais:

- `imagine-shell` — shell executável (`/bin/shell`);
- `imagine-bootloader` — binário UEFI do Limine e `limine.conf`;
- `imagine-video-server` — servidor de vídeo Liminal (`/bin/liminal-server`);
- `imagine-compositor` — compositor/window manager (`/bin/liminal-client`);
- `imagine-desktop` — ambiente de desktop (`/bin/ede-panel`);
- `dnu` — kernel e ferramentas do sistema (dzImage, DEF, DFS);
- `imagine-common` — código compartilhado (ABI, API, libc, runtimes, programas init/getty, RAMFS, documentação);
- `imagine-tools` — programas específicos (shell, Liminal), utilitários, scripts de build, isobuilder.

## Drivers

Os drivers são módulos nativos do kernel DNU (em `dnu/src/`: `ata`, `block`,
`keyboard`, `mouse`, `acpi`, `time`, `framebuffer`), com um hub genérico que
publica dispositivos no init. O antigo ecossistema de daemons de drivers
herdado do Redox (`drivers/` com `acpid`, `audio`, `nvme`, `usb`, …) foi
removido; os pedaços reaproveitáveis vivem agora no kernel. Detalhes em
[imagine-common/documentation/drivers.md](imagine-common/documentation/drivers.md).

## Estado do Boot

O kernel consome o HHDM, o mapa de memória, o framebuffer e o módulo RAMFS do Limine. Ele instala a GDT/TSS e uma IDT de exceções fatais, inicializa um alocador de frames de 4 KiB e um heap bump de 1 MiB, monta o arquivo USTAR e carrega `/sbin/init` como PID 1. O init inicia o `/sbin/getty`, que inicia o `/bin/shell`; o shell resolve comandos externos em `/bin`.

O código-fonte do kernel fica no repositório DNU (`dnu/`, checkout em `dnu/src/`; o Limine carrega o dzImage, que
é o próprio ELF do kernel, direto na RAM) e os aplicativos de userspace em
`imagine-common/programs/` (apps compartilhados como init e getty) e
`imagine-tools/programs/` (apps específicos como shell e Liminal) e
`imagine-tools/utilities/` (comandos de arquivo único). Programas de userspace
são empacotados no formato nativo **DEF** (magic `DNUF`) por `dnu-pack` — o
kernel não carrega ELF de userspace. O USTAR provê o sistema de arquivos
inicial em RAM e o fallback; quando existe uma partição DFS válida, sua árvore
persistente é usada como raiz ativa. Ainda não há uma implementação genérica
de `switch_root`.

O Getty autentica `root`/`root` ou a conta opcional criada pelo
`distroinstall`. O instalador pode definir o hostname e conceder acesso
administrativo a essa conta. Permissões de arquivo dono/grupo/outros impostas
pelo kernel, `su` e `sudo` estão disponíveis. A senha padrão do root continua
sendo `root`; troque-a apenas quando existir um mecanismo seguro e persistente
de gerenciamento de senhas.

> **Aviso:** Não exponha uma imagem compilada na rede ou a usuários não
> confiáveis enquanto a credencial `root` padrão estiver ativa. Qualquer pessoa
> que consiga alcançar o sistema pode entrar como `root` com a senha
> amplamente conhecida, e a credencial ainda não pode ser alterada de forma
> persistente. Mantenha essas imagens em máquinas isoladas e offline até que um
> mecanismo persistente de gerenciamento de senhas substitua o padrão embutido
> no caminho de autenticação do getty (o handler de login do getty em
> `imagine-common/programs/login/`, que hoje compara a senha digitada com o par
> `root`/`root` embutido no código).

O shell é propositalmente pequeno. Suas capacidades são:

- **Comandos internos:** `cd`, `pwd`, `echo`, `export`, `unset`, `set`, `read`,
  `clear`, `pid`, `type` e `exit`.
- **Utilitários externos:** os comandos são resolvidos via `PATH` e iniciados a
  partir de `/bin`; os utilitários incluídos são `ls`, `cat`, `grep` (busca por
  string exata), `mkdir`, `touch`, `rm`, `vi` e `globalconf`. O `argv` e as
  variáveis de ambiente exportadas são repassados aos programas filhos.
- **Fontes:** fontes PSF/PSF2 são procuradas em `ramfs/system/fonts`; glifos
  ASCII/portugueses embutidos são usados quando uma fonte carregada não tem
  determinado caractere. O cursor do prompt pisca enquanto a entrada é
  consultada.
- **Não suportado:** pipelines, redirecionamento, aliases, funções e sintaxe de
  controle de fluxo — este não é um shell completo no padrão POSIX.

O `std` do Rust é desnecessário: o kernel permanece `no_std` e expõe operações de sistema por meio de suas próprias syscalls. A base USTAR é imutável; `mkdir`, `touch` e `rm` atualizam uma camada em memória de tamanho limitado, e as mudanças desaparecem ao reiniciar. O escalonamento é cooperativo round-robin; preempção por timer, recuperação de heap e permissões completas de W^X também estão pendentes.

Programas Rust de usuário podem usar o crate `no_std` reutilizável em
`imagine-common/userland/api/`. Os números de syscall e as estruturas de dados
compartilhadas ficam em `imagine-common/abi/`; a ABI v2 e suas convenções de
erro e argumentos estão documentadas em
[imagine-common/documentation/runtime.md](imagine-common/documentation/runtime.md). Essa API de
userspace é específica do ImagineOS e não oferece `std` do Rust nem
compatibilidade POSIX.

Mais detalhes:
[imagine-common/documentation/README.md](imagine-common/documentation/README.md), incluindo o
[fluxo de boot](imagine-common/documentation/build.md) e o
[guia de desenvolvimento de programas de userspace](imagine-common/documentation/runtime.md).

## Copyright

    Copyright (C) 2024-2026 The Imagine Project
    
    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
