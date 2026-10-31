//! Extensões de plataforma do ImagineOS (`target_os = "dreamcore"`).
//!
//! É um **subconjunto** da camada Linux: o kernel expõe arquivos, threads e
//! os tipos crus de `raw` (fd, `uid_t`, `pid_t`), mas não tem rede, IPC SysV,
//! `pidfd` nem `/proc`. Incluir os módulos a mais faria a `std` compilar
//! chamadas que a musl não pode atender.

pub mod fs;
pub mod raw;
