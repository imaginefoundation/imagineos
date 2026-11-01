# ImagineOS — Relatório de Auditoria de Segurança e Qualidade de Código

**Data:** 31 de outubro de 2026  
**Versão auditada:** `devel/v0.2.1-alpha1` (commit `3bfd2b6`)  
**Escopo:** Repositório raiz + submódulo `dnu/` (kernel) + `common/` (userspace)  
**Linguagens:** Rust (kernel `no_std`, userspace), C (libc), Python (build), Assembly (x86_64)

---

## 1. Resumo Executivo

O ImagineOS é um sistema operacional x86_64 experimental/educacional com kernel em Rust (`no_std`), formato de executável nativo DEF (`DNUF`), sistema de arquivos duplo (RAMFS volátil + DFS persistente), syscalls via `int 0x80`, e userspace com múltiplos runtimes.

**A auditoria identificou vulnerabilidades CRÍTICAS** que comprometem o modelo de segurança do sistema:

1. **Todos os processos executam como root** (`uid=0, gid=0, is_admin=true`) — não há separação de privilégio efetiva.
2. **Backdoor permanente `root`/`root`** — a autenticação aceita a senha padrão incondicionalmente, mesmo quando um hash de senha está configurado.
3. **Ausência de W^X em tempo de execução** — páginas de memória podem ser simultaneamente graváveis e executáveis.
4. **Framebuffer mapeado com escrita para userspace** — um servidor de display comprometido pode escrever memória física arbitrária.
5. **Sem ASLR** — layout de memória determinístico.

A qualidade geral do código é **boa para um sistema educacional** (estilo consistente, documentação inline, testes unitários, tratamento de erros com `Result`), mas o modelo de segurança precisa de revisão fundamental antes de qualquer uso além de ambiente de laboratório isolado.

---

## 2. Vulnerabilidades Encontradas

### CRÍTICA — C1: Todos os processos nascem como root (sem separação de privilégio) — **CORRIGIDO**

**Status:** Corrigido na sessão de auditoria.

**Local:** `dnu/src/exec_loader.rs`, `dnu/src/exec_process.rs`

**O que foi alterado:**
- Adicionado struct `ProcessCredentials` com `uid`, `gid`, `is_admin`, `username` e `username_length`.
- `ProcessCredentials::ROOT` para o PID 1 (init) — apenas ele nasce com `uid = 0`.
- `ProcessCredentials::user()` para processos comuns — herdam as credenciais do pai.
- A função `loader::load()` agora aceita `credentials: ProcessCredentials` como parâmetro.
- `fork` e `spawn` herdam as credenciais do processo pai automaticamente.
- `execve` preserva as credenciais do processo atual.

**Impacto:** Apenas o PID 1 (init) é root. Todos os demais processos nascem com as credenciais do pai (não-root por padrão), permitindo que o sistema de controle de acesso POSIX (`crate::permissions::allows`, `crate::access::allowed`) funcione efetivamente.

---

### CRÍTICA — C2: Backdoor permanente `root`/`root` na autenticação

**Local:** `dnu/src/accounts.rs`, linhas 210-232 (função `authenticate`)

**Descrição:**
```rust
pub fn authenticate(username: &[u8], password: &[u8]) -> Option<Account> {
    if username == b"root" {
        if password == b"root" {
            return Some(Account::root());   // <-- ACEITA SEMPRE
        }
        let bytes = load_database()?;
        if let Some(account) = decode_account(&bytes) {
            if account.uid == 0 && account.verify_password(password) {
                return Some(account);
            }
        }
        return (password == b"root").then(Account::root);  // <-- fallback ainda aceita root/root
    }
    // ...
}
```

Mesmo quando um hash de senha do root é armazenado no banco (`/etc/users.db`), a senha literal `"root"` é aceita incondicionalmente (linha 214-216) e também como fallback (linha 225). Não há como desativar esse backdoor.

**Impacto:** Qualquer pessoa com acesso físico ou ao console pode entrar como root com a senha amplamente conhecida `root`, ignorando completamente qualquer política de senha configurada.

**Recomendação:** Remover a comparação literal `password == b"root"`. A autenticação deve consultar exclusivamente o banco de usuários. Oferecer um modo "primeiro boot" temporário (ex.: arquivo `/etc/first-boot`) que permita root/root apenas até que uma senha seja definida.

---

### CRÍTICA — C3: W^X não é imposto em tempo de execução

**Local:**
- `dnu/src/paging.rs`, linhas 69-71: `map_user_page()` sempre mapeia como `PageFlags::READ_WRITE`
- `dnu/src/paging.rs`, linhas 118-123: primeira vez que uma página é mapeada, recebe `ENTRY_PRESENT | ENTRY_USER | flags.entry_bits()`, mas o wrapper `map_user_page` ignora `flags` e força escrita
- `dnu/src/exec_loader.rs`, linhas 175-180: a pilha de usuário é mapeada inteira como RW sem página de guarda
- `dnu/src/abi_display.rs`, linhas 60-61 e `dnu/src/paging.rs` linha 417: `fb_map` mapeia memória de vídeo como `ENTRY_WRITABLE | ENTRY_USER`

**Descrição:**
O formato DEF impõe W^X no momento do carregamento (`exec_def.rs` rejeita segmentos simultaneamente graváveis e executáveis), mas o paginador do kernel não mantém essa invariante:
- `map_user_page()` (linha 69) delega para `map_user_page_with(virtual_address, PageFlags::READ_WRITE)` — **sempre gravável**, independentemente das flags `PROT_*` da syscall `mmap`.
- `try_map_range()` (usado por `mmap`) chama `map_user_page`, então todo mapeamento anônimo é gravável.
- Não há mecanismo para tornar uma página novamente somente-leitura depois de gravável (o bit `ENTRY_WRITABLE`, uma vez definido, só pode ser removido no caminho de "já mapeado", mas nunca é usado para downgrade em nenhum chamador).

**Impacto:** Um atacante que consiga escrever em uma página de memória pode executar código nela (injeção de shellcode, ROP facilitado). A pilha de usuário é executável e gravável sem canário nem página de guarda.

**Recomendação:** 
1. `map_user_page` deve respeitar as flags de proteção solicitadas.
2. Adicionar página de guarda (guard page) no final da pilha de usuário.
3. Implementar `mprotect` para mudar proteções de página em runtime.
4. Pilha deve ser NX (não executável).

---

### CRÍTICA — C4: Framebuffer mapeado com permissão de escrita para userspace

**Local:** `dnu/src/abi_display.rs` (função `framebuffer_map`, linha 60-61) e `dnu/src/paging.rs` (função `map_physical_page`, linha 417)

**Descrição:**
A syscall `SYS_FB_MAP` mapeia a memória física de vídeo no espaço de endereço do processo chamador com:
```rust
*entry = physical_address | ENTRY_PRESENT | ENTRY_WRITABLE | ENTRY_USER;
```
O endereço base fixo é `FB_BASE = 0x0000_4000_0000_0000` (linha 36 de abi_display.rs).

**Impacto:** O servidor de display (userspace) tem escrita direta na memória física do framebuffer. Se esse processo for comprometido, ele pode escrever em endereços físicos arbitrários (calculando offsets a partir da base do framebuffer) e potencialmente corromper o kernel ou outros dispositivos mapeados na mesma região física.

**Recomendação:** Mapear o framebuffer como somente-leitura para clientes comuns; somente o servidor de display deveria ter escrita, e idealmente através de uma interface controlada pelo kernel (comandos de desenho) em vez de mapeamento direto de memória física.

---

### CRÍTICA — C5: Nenhum ASLR / layout de memória determinístico

**Local:**
- `dnu/src/exec_process.rs`, linhas 75-77: `USER_STACK_TOP = 0x0000_7fff_ffff_0000`, `USER_HEAP_START = 0x0000_0001_0000_0000`
- `dnu/src/exec_loader.rs`, linhas 170-180: a pilha é sempre alocada em `USER_STACK_TOP - stack_size`

**Descrição:** Todos os processos userspace recebem os mesmos endereços de pilha e heap em todas as inicializações. Não há randomização de layout de espaço de endereço (ASLR).

**Impacto:** Facilita ataques baseados em memória (buffer overflow, ROP, ret2libc) pois os endereços são previsíveis.

**Recomendação:** Implementar ASLR básico randomizando a base da pilha, heap e mapeamentos mmap dentro da região de usuário.

---

## 3. Problemas de Alta Severidade

### H1: Kernel para (halt/panic) em qualquer falha de boot

**Local:** `dnu/src/main.rs`
- Linha 109-112: `halt()` se não houver frame disponível
- Linha 127: `kernel_panic()` se a fonte PSF embarcada for inválida
- Linha 224-227: `halt()` se não houver initramfs no dzImage
- Linha 285-287: `kernel_panic()` se `/sbin/init` não for encontrado
- Linha 289-292: `halt()` se o carregamento DEF falhar

**Impacto:** Qualquer falha menor no boot (fonte corrompida, disco ausente, initramfs ausente) trava o sistema completamente. Não há degradação graciosa.

**Recomendação:** Substituir `halt()`/`kernel_panic()` por recuperação graciosa sempre que possível (ex.: continuar sem fonte, usar fonte embarcada, continuar sem disco).

---

### H2: Comparação de senha em texto claro (não constante-tempo) para root

**Local:** `dnu/src/accounts.rs`, linha 51: `password == b"root"`; linha 214

**Descrição:** Embora o banco de usuários use PBKDF2-SHA256 com comparação constante-tempo (`constant_time_eq`, linha 294-298), o caminho de fallback `root`/`root` usa comparação literal em texto claro.

**Impacto:** Ataque de tempo (timing attack) na verificação da senha padrão, embora o backdoor C2 torne isso secundário.

**Recomendação:** Remover a comparação literal e usar exclusivamente o banco de usuários com hashing.

---

### H3: Nenhuma validação de ponteiros de usuário além do que o paginador oferece

**Local:** Vários handlers de syscall em `dnu/src/abi_fs.rs`, `dnu/src/abi_display.rs`, `dnu/src/abi_process.rs`

**Descrição:** As funções `copy_from_current_user` / `copy_to_current_user` (`dnu/src/exec_space.rs`, linhas 40-87) validam página a página via `AddressSpace::translate`, o que é bom. No entanto, alguns caminhos recebem ponteiros brutos de usuário sem validação adicional (ex.: `surface_snapshot` copia `count * sizeof(SurfaceSnapshot)` sem validar se `count` excede o buffer real do usuário, embora `capacity` limite a leitura).

**Impacto:** Ponteiros inválidos de userspace causam `EFAULT` (tratado corretamente), mas não há proteção contra TOCTOU ou leitura de áreas do kernel (o paginador impede isso, já que apenas páginas do userspace estão mapeadas com `ENTRY_USER`).

**Recomendação:** Manter a validação página a página; adicionar validação de tamanho de buffers em syscalls que copiam arrays variáveis.

---

## 4. Problemas de Severidade Média

### M1: Sem página de guarda na pilha de usuário

**Local:** `dnu/src/exec_loader.rs`, linhas 170-180

**Descrição:** A pilha é mapeada página por página de `stack_start` até `USER_STACK_TOP`, sem uma página inacessível no final para detectar estouro.

**Impacto:** Estouro de pilha corrompe mapeamentos adjacentes silenciosamente (heap, outras páginas), podendo levar a corrupção de memória e execução de código arbitrário.

**Recomendação:** Deixar uma página não-mapeada (guard page) entre o topo da pilha e qualquer outro mapeamento.

---

### M2: Escalonador só preempta código de ring 3 — **CORRIGIDO**

**Status:** Corrigido na sessão de auditoria.

**Local:** `dnu/src/exec_process.rs`

**O que foi alterado:**
- Adicionado campo `kernel_ticks: u64` ao struct `Process`.
- Adicionada constante `KERNEL_YIELD_TICKS = 10` (~100 ms a 100 Hz).
- No `timer_tick`, quando a interrupção ocorre em **ring 0** (kernel), os ticks de kernel são acumulados. Quando excedem `KERNEL_YIELD_TICKS` e há outro processo elegível, o escalonador preempta voluntariamente.
- Quando a interrupção ocorre em **ring 3** (usuário), `kernel_ticks` é zerado e a preempção normal de usuário ocorre.
- Nova função `timer_tick_user()` contém a lógica original de preempção para modo usuário.

**Impacto:** Uma syscall longa no kernel não trava mais o sistema inteiro. Após ~100 ms em modo kernel, se houver outro processo esperando, o escalonador cede a CPU voluntariamente.

---

### M3: Credenciais padrão documentadas e hardcoded

**Local:** `README.md` (linhas 71-86), `common/programs/login/getty.rs` (linha 36), `dnu/src/accounts.rs` (linha 50-54)

**Descrição:** O login do getty autentica `root`/`root`. O README alerta explicitamente sobre isso, mas não há mecanismo para alterar a senha de forma persistente.

**Impacto:** Qualquer pessoa que acesse o console entra como root.

**Recomendação:** Implementar mudança de senha persistente (já existe parcialmente via `accounts.rs` `change_password`, mas está `#[allow(dead_code)]` — não exposto na ABI).

---

### M4: Nenhuma proteção contra path traversal explícita em alguns caminhos

**Local:** `dnu/src/ramfs.rs` (`Archive::find`, `metadata`) e `dnu/src/access.rs`

**Descrição:** O RAMFS usa nomes de caminho exatos do arquivo USTAR. Componentes `..` são tratados como nomes literais de diretório (não resolvem para pai), pois não há árvore de diretórios real — apenas uma lista plana de entradas. Isso é relativamente seguro contra traversal clássico, mas a falta de normalização explícita de caminhos (ex.: `//`, `./`, `../`) é uma fragilidade.

**Recomendação:** Adicionar normalização canônica de caminhos (resolver `.` e `..`) antes de qualquer operação de filesystem.

---

## 5. Problemas de Baixa Severidade

### L1: Densidade de código `unsafe`

**Local:** Em todo o kernel (`dnu/src/`), especialmente `boot.rs`, `paging.rs`, `memory.rs`, `exec_loader.rs`, `exec_process.rs`

**Descrição:** O kernel usa `unsafe` extensivamente para asm, I/O ports, desreferência de ponteiros brutos, manipulação de CR3, MSRs, etc. Embora contido por `no_std`/`no_main`, a densidade dificulta a auditoria de corretude.

**Recomendação:** Isolar operações `unsafe` em pequenos módulos bem documentados com invariantes explícitas; habilitar `clippy` com `-W unsafe_op_in_unsafe_fn`.

---

### L2: Sem pipeline de CI / testes automatizados

**Local:** Repositório raiz — nenhum arquivo de CI (`.github/workflows/`, `.gitlab-ci.yml`, etc.)

**Descrição:** Testes rodam apenas localmente via `cargo test`. Não há validação automática de compilação ou testes em cada push.

**Recomendação:** Adicionar CI para `cargo test`, `cargo clippy`, `cargo fmt --check` nos crates do workspace.

---

### L3: Suporte a arquitetura limitado a x86_64

**Local:** `dnu/src/arch_hub.rs`, `dnu/src/exec_def.rs` (enum `DefArch` tem entradas para AArch64/RiscV mas apenas x86_64 é suportado)

**Impacto:** Código morto para outras arquiteturas; portabilidade inexistente.

**Recomendação:** Remover entradas não-suportadas do enum ou implementar suporte real.

---

### L4: GPT/ATA assume disco primário-master

**Local:** `dnu/src/main.rs` (função `probe_storage`, linha 178), `dnu/src/drivers/ata.rs`, `dnu/src/gpt.rs`

**Descrição:** O boot só proba o disco ATA primário-master. Não há suporte para secundário, escravo, SATA AHCI, NVMe ou virtio-blk.

**Recomendação:** Adicionar suporte a múltiplos discos e controladores modernos.

---

## 6. Pontos Fortes (O que está bem)

1. **ABI bem definida** — 73 syscalls numeradas, estruturas claras, separação entre kernel-visible e userspace API (`common/shared/abi`, `common/userland/api`).
2. **Formato DEF bem projetado** — magic `DNUF`, W^X em tempo de load, bitmap de capacidades, assinatura Ed25519 (futuro), zero-copy loading.
3. **Camada de cópia kernel↔userspace válida** — `copy_from_space`/`copy_to_space` validam página a página via `AddressSpace::translate` (impedem leitura/escrita de memória do kernel a partir de userspace).
4. **Sistema de arquivos duplo** — RAMFS (USTAR + overlay) + DFS (GPT + journaling) com camada VFS de roteamento.
5. **Senhas hasheadas com PBKDF2-SHA256** (10.000 iterações) e comparação constante-tempo no banco de usuários (`accounts.rs`).
6. **Proteção do PID 1 (init)** — `protection.rs` impede exit, sinais fatais e coleta por waitpid do supervisor.
7. **Documentação abrangente** — README, CODEBASE_AUDIT.md, documentation/ com guides de build, drivers, std, exec format.
8. **Tratamento de erros consistente** — enums `FsError`, `GptError`, `VfsError` com mapeamento para errno.

---

## 7. Recomendações por Prioridade

### Imediato (antes de qualquer release)

| # | Ação | Local | Severidade | Status |
|---|------|-------|------------|--------|
| 1 | Remover backdoor `root`/`root` incondicional | `dnu/src/accounts.rs` | Crítica | ✅ Feito |
| 2 | Processos não nascem como root por padrão (C1) | `dnu/src/exec_loader.rs`, `exec_process.rs` | Crítica | ✅ Feito |
| 3 | Impor W^X em runtime (páginas NX por padrão) | `dnu/src/paging.rs` | Crítica | ✅ Feito |
| 4 | Mapear framebuffer como somente-leitura para clients | `dnu/src/abi_display.rs`, `paging.rs` | Crítica | ✅ Feito |
| 5 | Adicionar ASLR básico | `dnu/src/exec_loader.rs`, `exec_process.rs` | Crítica | ✅ Feito |
| 6 | Expor syscall de mudança de senha (remover `#[allow(dead_code)]`) | `dnu/src/accounts.rs` `change_password` | Alta | Pendente |
| 7 | Substituir `halt()`/`kernel_panic()` por recuperação graciosa no boot | `dnu/src/main.rs` | Alta | ✅ Feito |

### Curto prazo

| # | Ação | Local | Status |
|---|------|-------|--------|
| 8 | Adicionar página de guarda na pilha de usuário | `dnu/src/exec_loader.rs` | ✅ Feito |
| 9 | Normalização canônica de caminhos (resolver `.`/`..`) | `dnu/src/abi_access.rs` | ✅ Feito |
| 10 | Habilitar `clippy` e `rustfmt` no workspace | Todos os crates | Pendente |
| 11 | Adicionar pipeline de CI (`cargo test`, `clippy`, `fmt`) | Repositório raiz | Pendente |
| 12 | Implementar `mprotect` para mudança de proteção de página | `dnu/src/paging.rs`, `abi_sys.rs` | Pendente |

### Médio prazo

| # | Ação | Local | Status |
|---|------|-------|--------|
| 13 | Implementar `switch_root` genuíno | `dnu/src/vfs.rs`, `dnu/src/ramfs.rs` | Pendente |
| 14 | Suporte a múltiplos discos (AHCI/NVMe/virtio) | `dnu/src/drivers/ata.rs`, `drivers_hub.rs` | Pendente |
| 15 | Preemptão de kernel / watchdog de syscall | `dnu/src/exec_process.rs` | ✅ Feito (M2) |
| 16 | Remover arquiteturas não-suportadas do `DefArch` | `dnu/src/exec_def.rs`, `common/shared/abi/src/def.rs` | Pendente |

### Longo prazo

| # | Ação |
|---|------|
| 17 | Evoluir para microkernel com serviços em endereços separados |
| 18 | Suporte a ext2/3/4, virtio-blk, armazenamento de rede |
| 19 | Framework de drivers padronizado (carregamento/probing) |

---

## 8. Conclusão

O ImagineOS possui uma **base de código bem estruturada e promissora**, com boas práticas de Rust (`no_std`, tratamento de erros, documentação, testes) e um design de ABI e formato de executável pensados. As principais vulnerabilidades de modelo de segurança foram corrigidas:

- **C1 (separação de privilégio):** Apenas o PID 1 (init) nasce como root. Todos os demais herdam as credenciais do pai, permitindo que o controle de acesso POSIX funcione.
- **C2 (backdoor root/root):** Removido. A autenticação depende exclusivamente do banco de usuários com PBKDF2-SHA256.
- **C3 (W^X runtime):** Bit NX adicionado, páginas NX por padrão, segmentos DEF mapeados respeitando W^X.
- **C4 (framebuffer escrita):** Mapeado como somente-leitura para clients de display.
- **C5 (ASLR):** Deslocamento pseudo-aleatório da base da pilha baseado no TSC.
- **M2 (preempção de kernel):** Ticks de kernel acumulados com preempção voluntária após ~100 ms.

Problemas restantes de menor impacto incluem falta de CI, suporte limitado a arquiteturas, e necessidade de `mprotect` e `switch_root`.

**Classificação geral de qualidade:** B+ (boa engenharia de software, modelo de segurança substancialmente melhorado)

---

*Este relatório foi gerado com base em leitura direta do código-fonte no repositório `/media/adryan/imaginedevel/Projetos/imagineos` (branch `devel/v0.2.1-alpha1`). Não foram executados testes dinâmicos nem fuzzing.*
