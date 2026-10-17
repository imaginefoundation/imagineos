//! Formato e execução de imagens: parsers (DEF/ELF), loader e escalonador.
//!
//! `process` é o módulo do escalonador; `descriptors`, `loader` e `space` são
//! seus blocos de apoio. Os parsers ficam em módulos próprios porque não
//! dependem de nenhum outro subsistema do kernel.

#[path = "def.rs"]
pub mod def;
pub mod descriptors;
pub mod elf;
pub mod loader;
pub mod process;
pub mod space;
