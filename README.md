# Imagine Operating System (astrid-alpha1-v0.2.1)

O ImagineOS é um sistema operacional x86_64 experimental e educacional, de codinome **Astrid**. O kernel é Rust freestanding e usa o protocolo de boot do Limine. Este é um estágio inicial de bring-up, ainda não é um sistema operacional de uso geral.

## Compilação

Requisitos: Rust stable com o alvo `x86_64-unknown-none`, GNU Make, o executável UEFI x86_64 do Limine em `third_party/limine/BOOTX64.EFI`, `xorriso`, `dosfstools`, `mtools` e QEMU com OVMF.

O QEMU com OVMF é necessário tanto para a suíte de testes (`make test`) quanto
para testar o boot manualmente pela ISO. `xorriso`, `dosfstools` e `mtools`
só são necessários para gerar a ISO inicializável; a suíte de testes não
precisa deles. Nenhuma ferramenta além das citadas é necessária para rodar os
testes.

```sh
rustup target add x86_64-unknown-none
make ramfs
make kernel
make iso
```

O diretório `ramfs/` na raiz do repositório é a árvore de origem usada para
montar o arquivo do sistema de arquivos, portanto ele precisa existir antes de
rodar `make iso` (o trecho de comandos acima já o cria). A imagem somente UEFI
é gravada em `.build/distro/dreamcore-YYYY-MM-DD-HH-MM-astrid.iso`.

## Estado do Boot

O kernel consome o HHDM, o mapa de memória, o framebuffer e o módulo RAMFS do Limine. Ele instala a GDT/TSS e uma IDT de exceções fatais, inicializa um alocador de frames de 4 KiB e um heap bump de 1 MiB, monta o arquivo USTAR e carrega `/sbin/init` como PID 1. O init inicia o `/sbin/getty`, que inicia o `/bin/shell`; o shell resolve comandos externos em `/bin`.

O código-fonte do kernel fica no repositório DNU (`dnu/`, checkout em `dnu/src/`; o Limine carrega o dzImage, que
é o próprio ELF do kernel, direto na RAM) e os aplicativos de userspace em
`common/programs/` (apps multi-módulo) e `common/utilities/` (comandos de
arquivo único). Programas de userspace são empacotados no formato nativo **DEF**
(magic `DNUF`) por `dnu-pack` — o kernel não carrega ELF de userspace. O USTAR provê o
sistema de arquivos inicial em RAM e o fallback; quando existe uma partição DFS
válida, sua árvore persistente é usada como raiz ativa. Ainda não há uma
implementação genérica de `switch_root`.

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
> `common/programs/login/`, que hoje compara a senha digitada com o par
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
`common/userland/api/`. Os números de syscall e as estruturas de dados compartilhadas
ficam em `common/shared/abi/`; a ABI v2 e suas convenções de erro e argumentos estão
documentadas em
[documentation/syscall-abi.md](documentation/syscall-abi.md). Essa API de
userspace é específica do ImagineOS e não oferece `std` do Rust nem
compatibilidade POSIX.

Mais detalhes: [documentation/README.md](documentation/README.md), incluindo o
[fluxo de boot](documentation/boot-flow.md) e o
[guia de desenvolvimento de programas de userspace](documentation/userspace.md).

## Copyright

    Copyright (C) 2024-2026 The Imagine Project & Adryan Alcantara
    
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
