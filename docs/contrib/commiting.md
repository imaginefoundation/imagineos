# Convenção de Commits — ImagineOS

Este documento define o padrão de mensagens de commit para o repositório do
ImagineOS (kernel DNU, userspace e ferramentas de build). Ele é uma adaptação
da especificação [Conventional Commits](https://www.conventionalcommits.org/)
ajustada à estrutura e ao idioma do projeto.

> **Nota:** O idioma padrão das mensagens de commit é **português do Brasil**,
> já que a maior parte do histórico e da documentação do projeto usa esse idioma.

---

## Formato da mensagem

```
<tipo>(<escopo opcional>): <descrição>

<corpo opcional>

<rodapé opcional>
```

Onde:

- **`<tipo>`**: obrigatório. Indica a natureza da alteração.
- **`<escopo>`**: opcional. Indica a área do projeto afetada.
- **`<descrição>`**: obrigatório. Resumo conciso no imperativo presente, sem letra maiúscula inicial e sem ponto final.
- **`<corpo>`**: opcional. Motivação e contraste com o comportamento anterior.
- **`<rodapé>`**: opcional. Referências a issues e indicação de *breaking changes*.

Exemplo:

```
feat(kernel): adicionar suporte a syscall de framebuffer

Implementa SYS_FB_INFO e SYS_FB_MAP para permitir que o ws_server
mapeie o framebuffer via userspace.

BREAKING CHANGE: o número da SYS_FB_MAP mudou de 49 para 50.
```

---

## Tipos permitidos

| Tipo | Quando usar | Exemplo |
|------|-------------|---------|
| `feat` | Nova funcionalidade para kernel ou userspace | `feat(gui): adicionar janela de bloco de notas` |
| `fix` | Correção de bug | `fix(kernel): corrigir page fault em mount_root_filesystems` |
| `docs` | Alterações apenas em documentação | `docs: atualizar AGENTS.md com novo caminho da ABI` |
| `build` | Build system, scripts, dependências, CI | `build: atualizar makefile do DNU para incluir dnu-pack` |
| `refactor` | Reestruturação sem mudar comportamento externo | `refactor: mover ABI para dnu/abi/` |
| `perf` | Melhoria de desempenho (tipo especial de refactor) | `perf(mm): reduzir iterações no alocador de frames` |
| `style` | Formatação, espaçamento, ponto e vírgula (sem mudar comportamento) | `style: padronizar indentação em main.rs` |
| `test` | Adição ou correção de testes | `test: adicionar teste de parse DEF no loader` |
| `chore` | Tarefas rotineiras (.gitignore, inicialização, manutenção) | `chore: atualizar .gitignore para ignorar target/` |
| `security` | Correções ou melhorias de segurança | `security: adicionar PBKDF2 ao banco de credenciais` |
| `ops` | Infraestrutura, deploy, CI/CD, monitoramento | `ops: adicionar workflow de build no GitHub Actions` |

---

## Escopos sugeridos

O escopo ajuda a localizar rapidamente a área afetada. Use um dos seguintes quando aplicável:

| Escopo | Área |
|--------|------|
| `kernel` | Código do kernel DNU (`dnu/src/`) |
| `abi` | Contrato de ABI (`dnu/abi/`, `dnu/src/abi/`) |
| `mm` | Gerenciamento de memória e paginação (`dnu/src/mm/`) |
| `fs` | Sistemas de arquivos (RAMFS, DFS, VFS) |
| `process` | Carga de DEF, escalonador, syscalls de processo |
| `drivers` | Drivers de bloco, entrada, vídeo, platform |
| `boot` | Bootloader Limine, configuração de boot |
| `userspace` | Programas em `imagine-common/` e `imagine-tools/` |
| `gui` | Interface gráfica (`ws_server`, `wm`, `gdi`, `explorer`) |
| `build` | Scripts de build (`imagine-tools/build/`, `build.py`) |
| `docs` | Documentação (`docs/`, `AGENTS.md`, `README.md`) |

Exemplos com escopo:

```
fix(kernel): corrigir GPF ao acessar dzimage com HHDM offset
feat(gui): adicionar suporte a SYS_SURFACE_CREATE
build(userland): injetar dependência dnu-abi nos manifests
docs(abi): documentar novos números de SYS_*
```

---

## Breaking Changes

Um commit que introduz alterações que quebram compatibilidade **deve** usar o
indicador `!` antes dos dois pontos no tipo:

```
feat(abi)!: remover SYS_INT_0x80 em favor de syscall/sysret
```

O rodapé deve descrever o impacto:

```
BREAKING CHANGE: todos os programas userspace devem ser recompilados
e usar a instrução `syscall` em vez de `int 0x80`.
```

---

## Descrição

- Use o **imperativo, presente**: "adicionar" não "adicionado" nem "adiciona".
- Não capitalize a primeira letra.
- Não termine com ponto (`.`).
- Seja conciso (máximo recomendado: 72 caracteres).

Exemplos corretos:

```
feat: adicionar suporte a syscall de framebuffer
fix(kernel): corrigir wraparound em embedded_initramfs
```

Exemplos incorretos:

```
feat: Adicionado suporte a syscall de framebuffer
fix(kernel): Corrigido o bug do page fault.
docs: Atualiza a documentação da ABI.
```

---

## Corpo e Rodapé

O corpo é opcional e deve explicar o **porquê** da mudança, não apenas o **o quê**.

O rodapé é opcional, exceto para *breaking changes*. Pode conter referências a
issues (ex: `Closes #123`, `Fixes #456`) e deve iniciar com `BREAKING CHANGE:`
quando houver alteração que quebra compatibilidade.

---

## Exemplos práticos para o ImagineOS

```bash
# Nova funcionalidade no kernel
git commit -m "feat(kernel): adicionar handler de page fault ao IDT"

# Correção no build do userspace
git commit -m "build(userland): corrigir injeção de dnu-abi no manifest"

# Documentação
git commit -m "docs: atualizar AGENTS.md com novo caminho da ABI"

# Breaking change na ABI
git commit -m "feat(abi)!: mudar SYS_FB_MAP de 49 para 50

BREAKING CHANGE: ws_server e init devem ser recompilados contra a nova ABI."
```

---

## Regras de ouro do ImagineOS

Lembre-se das regras definidas no `AGENTS.md`:

1. **Não modifique `DONOTCOMMIT/`** — contém arquivos binários de terceiros.
2. **GUI fica no userspace** — nunca coloque código de janela, compositor ou framebuffer no kernel DNU.
3. **Sempre teste no QEMU antes de commitar** — se o commit afeta o boot, rode `python3 build.py iso` e `python3 build.py run`.
4. **Mantenha o `no_std` no kernel** — não use `std` em `dnu/src/`.
5. **A ABI é contrato do kernel** — alterações em `dnu/abi/` afetam todo o userspace; documente no rodapé.

---

## Referências

- [Conventional Commits](https://www.conventionalcommits.org/)
- [Angular Commit Guidelines](https://github.com/angular/angular/blob/master/CONTRIBUTING.md)
- [AGENTS.md — Guia de Desenvolvimento do ImagineOS](../AGENTS.md)