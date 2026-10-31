# Contribuindo

- Kernel: Rust stable, `no_std`, target `x86_64-unknown-none`.
- Valide com `rustup run stable cargo check --target x86_64-unknown-none` e `rustup run stable cargo build --release --target x86_64-unknown-none`.
- O build é dividido por etapa em `build/`: `common.mk` (variáveis), `kernel.mk`
  (kernel, dzImage), `tools.mk` (ferramentas do host), `userland.mk` (apps,
  utilitários, libc, runtime C),
  `ramfs.mk` (empacotamento USTAR) e `iso.mk` (ISO e QEMU). O `Makefile` raiz só
  delega a cada um com `make -f`. Rode uma etapa isolada, por exemplo
  `make -f build/userland.mk user-programs`; ao mexer em uma etapa, mantenha as
  variáveis compartilhadas em `build/common.mk`, nunca duplicadas.
- Para testar o parser RAMFS junto com a camada VFS, rode a suíte do kernel a
  partir do checkout do DNU: `make -C dnu test` (cada arquivo de `dnu/tests/` é
  um harness próprio). O alvo isolado `dnu/src/ramfs.rs` não compila sozinho
  (depende de `crate::vfs`); use o teste de integração em `dnu/tests/vfs.rs`.
- Antes de gerar a imagem, garanta que exista `ramfs/` na raiz do projeto. Se a pasta não existir, crie-a com `mkdir -p ramfs`; ela é a árvore de origem do sistema de arquivos da imagem.
- O sistema de arquivos raiz da imagem vem de `ramfs/`; binários Rust de usuário são empacotados em `/bin` pelo alvo `make iso`.
- O kernel fica no repositório DNU (`dnu/`, checkout em `dnu/src/`); o loader é o próprio kernel, carregado direto pelo Limine; aplicativos de userspace, incluindo `init` (`sysman/`) e `getty` (`login/`), ficam em `common/programs/`. Utilitários em `common/utilities/` são copiados pelo build para `/bin`; `init` e `getty` são instalados em `/sbin` sem sufixo no nome.
- O kernel **só** carrega imagens DEF (magic `DNUF`); não reintroduza o loader ELF de userspace. O ELF só existe como saída intermediária do toolchain: `dnu-pack` o converte para DEF.
- Programas DEF não usam extensão de arquivo. Não use `.xapp` para executáveis — a extensão pertence ao pacote de aplicação do Liminal, que é outro artefato.
- Preserve os comandos internos (built-ins) e a busca por `PATH` no shell; não adicione comandos de escrita enquanto o RAMFS for somente leitura.
- Mudanças em boot, memória ou interrupções também precisam de teste da ISO em QEMU/OVMF antes de serem consideradas validadas.
- Não copie executáveis de usuário como arquivos independentes no ESP; o kernel os carrega do arquivo `ramfs.tar`.
