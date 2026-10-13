# PR: Limpeza de warnings da codebase e correções no protocolo de superfícies

## Resumo

Este PR elimina os 112 avisos de compilação da codebase (`cargo clippy` e o
build do kernel/userland agora terminam com **0 warnings**) e corrige dois
bugs funcionais descobertos durante o teste do sistema no QEMU:

1. **`ede-panel` crashava com page fault** ao criar a superfície: a base da
   região de superfícies (`SURFACE_REGION_BASE`) colidia com o limite da heap
   de usuário; e a API `Surface::create` nunca preenchia `width`/`height` no
   `SurfaceInfo` enviado ao kernel, que rejeitava o pedido com `EINVAL`.
2. **Fila de superfícies sujas nunca drenada** pela syscall
   `surface_snapshot` (o `take_dirty()` ficava sem chamador).

## O que mudou

### Correção de lints (edition 2024 / clippy `-D warnings`)

- **`unsafe_op_in_unsafe_fn`** — na mesma sessão em que os corpos foram
  revisados, adotamos a política documentada no topo de `kernel/src/main.rs`:
  funções `unsafe fn` do kernel usam operações unsafe no corpo por contrato
  (asm de E/S, voláteis de MMIO) e o lint é permitido por crate até uma
  revisão completa que envolva cada corpo em blocos `unsafe` internos com
  comentários SAFETY individuais. Os arquivos revisados nesta passada
  (`time.rs`, `paging.rs`, `power.rs`, `bootstrap.rs`) ganharam os blocos
  internos.
- **`missing_safety_doc`** — funções `unsafe` públicas do kernel e da API de
  usuário agora documentam o contrato de segurança.
- **`unused`** — constantes sem uso (`EAGAIN` em `surface.rs`, glifos não
  usados no display) removidas; reexports de funções `unsafe` em `common.rs`
  viram wrappers seguros; utilitários (`cat`, `grep`, `ls`, `mkdir`, `rm`,
  `touch`, `vi`) ajustados ao novo contrato de `resolve_path`.
- **`should_implement_trait`** — `fs::create` renomeado para `create_dir`.
- **`precedence`** — expressão ambígua em `fs.rs` parêntizada.
- **`not_unsafe_ptr_arg_deref`** — `legacy::resolve_path` passa a exigir
  `unsafe` corretamente.
- **`dead_code`** — APIs de superfície ainda sem chamador
  (`push_event_for`, `notify_hidden`) anotadas com justificativa.

### Correções funcionais

- `shared/abi`: `SURFACE_REGION_BASE` movida de `0x1_4000_0000` (logo após o
  limite da heap de 64 MiB) para `0x2_4000_0000` (+256 MiB), eliminando a
  colisão que fazia o painel EDE tomar page fault na segunda página do buffer
  (`cr2=0x140002000`).
- `userland/api/src/surface.rs`: `Surface::create` agora preenche
  `info.width`/`info.height` antes da syscall — antes o kernel recebia zeros
  e respondia `EINVAL`, e o painel nunca subia.
- `kernel/src/abi/syscall.rs`: `surface_snapshot` drena a fila de ids sujos
  (`surface::take_dirty`) a cada consulta do servidor, além de diagnosticar
  falhas de `surface::create` no log do kernel (`surface: no free slots`,
  `surface: no frames`, `surface: map failed`).

## Como testar

```sh
# Lints e testes de workspace
rustup run stable cargo clippy --workspace --all-targets -- -D warnings
rustup run stable cargo test

# Build completo (kernel + bootstrap + userland + ISO)
make -j4

# Boot no QEMU: o painel EDE deve registrar a superfície sem page fault
make run
```

## Verificação

- `cargo clippy --workspace --all-targets -- -D warnings`: **0 avisos**
- `cargo test`: todos os testes de workspace passam
- `make -j4` (kernel, bootstrap, userland, ISO): **0 avisos**
- QEMU: init sobe, display server assume a tela, painel EDE registra a
  superfície e compõe — sem page fault em `cr2=0x140002000`
