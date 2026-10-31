# musl no ImagineOS

A musl em `third_party/musl-1.2.6` foi portada para usar **somente** as syscalls
do Dreamcore. Não resta nenhuma instrução `syscall` do Linux nem número de
syscall do Linux no caminho de execução.

## Como a portabilidade está organizada

| Arquivo | Papel |
|---|---|
| `arch/x86_64/syscall_arch.h` | `__syscallN` delega para `__dreamcore_syscall` (ponte C) |
| `arch/x86_64/bits/syscall.h.in` | mapa `SYS_xxx → número do ImagineOS` (fonte única) |
| `src/internal/dreamcore.c` | executa `int 0x80`, traduz paths/flags/structs |
| `src/thread/x86_64/clone.s` | `clone()` → `SYS_THREAD_CREATE` |
| `src/thread/x86_64/__set_thread_area.s` | `arch_prctl(SET_FS)` → `SYS_SET_TLS` |
| `src/process/x86_64/vfork.s` | `vfork()` → `SYS_FORK` |
| `src/thread/x86_64/__unmapself.s` | `int 0x80` com `munmap`/`exit` |
| `src/signal/x86_64/restore.s` | `rt_sigreturn` → `SYS_SIGRETURN` |

### Por que um único ponto (`__dreamcore_syscall`)

A musl chama `__syscall(SYS_xxx, ...)` em centenas de lugares. Em vez de editar
cada um, todos passam por uma função C que:

1. recebe o número **já traduzido** (o `bits/syscall.h.in` é a tabela);
2. executa a ABI do kernel (`rax` = número, `rdi/rsi/rdx/r10/r8/r9` = args);
3. converte o que diverge — só o que diverge.

O caminho quente (`write`, `read`, `futex`) não ganha cópia nem alocação.

### `__NR_IGNORE` / `DC_IGNORE`

Syscalls que o ImagineOS não tem (rede, IPC SysV, epoll, `/proc`, credenciais)
recebem `DC_IGNORE` e a ponte devolve `-ENOSYS`. A musl já trata `ENOSYS` como
"recurso ausente" em quase todo lugar; é o mesmo caminho que ela usa em kernels
antigos.

## Validação

```sh
make musl            # compila third_party/musl-1.2.6 em lib/libc.a
```

Depois de compilar, o binário C de teste não contém nenhuma instrução `syscall`:

```sh
objdump -d hello | grep -cE "0f 05"   # 0
objdump -d hello | grep -c "int .*0x80"  # > 0
```

Os números emitidos são os do Dreamcore (`0x11` = `OPEN`, `0x42` = `FUTEX`,
`0x43` = `NANOSLEEP`, `0x44` = `CLOCK_GETTIME`, `0x45` = `MMAP`, `0x47` =
`SET_TLS`), não os do Linux.

## Limitações conhecidas

1. **Caminhos relativos.** O kernel não tem `cwd`; `open`/`stat` da musl só
   aceitam caminhos absolutos. O shell exporta `PWD` e a libc C do projeto o
   usa; a musl ainda não resolve relativos sozinha.
2. **Diretórios.** O kernel não tem descritores de diretório, então
   `getdents64` devolve `ENOTDIR`. `opendir`/`readdir` da musl não funcionam;
   a listagem é feita por `SYS_READDIR` na libc C do projeto.
3. **Rede e IPC SysV** não existem no kernel: todas respondem `ENOSYS`.
4. **`errno` por thread** depende de `SYS_SET_TLS` (implementada) + TLS da
   thread criada por `pthread_create` (o kernel grava `fs_base` por thread).
5. **O binário gerado é ELF.** O loader do kernel aceita DEF; programas C
   precisam passar por `dnu-pack` como os demais.
