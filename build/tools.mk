# ==============================================================================
# ImagineOS - ferramentas do host usadas pelo kernel e pela ISO
# ==============================================================================
#
# Compila os utilitários de host que o build consome (por exemplo, o formatador
# de imagem DFS usado por `disk-image`). Separar isto de kernel.mk deixa claro
# que são programas do host, não código que roda dentro do ImagineOS.
#
# Uso isolado:  make -f build/tools.mk
# Alvos:        tools
include $(dir $(lastword $(MAKEFILE_LIST)))common.mk

# O dnu-pack é um programa de host que precisa do crate imagineos-abi para
# reutilizar a definição do formato DEF (magic, header, validação). Como o
# .cargo/config.toml fixa o target padrão como x86_64-unknown-none (bare-metal),
# compilamos a abi explicitamente para o triple do host para que o binário de
# host possa linkar contra ela.
HOST_TRIPLE   := $(shell rustc -vV | sed -n 's/^host: //p')
ABI_MANIFEST  := $(ROOT)/common/shared/abi/Cargo.toml
ABI_TARGET_DIR := $(BUILD_DIR)/tools/abi-target
ABI_RLIB      := $(ABI_TARGET_DIR)/$(HOST_TRIPLE)/release/libimagineos_abi.rlib

# O dnu-pack converte o ELF que o toolchain emite para o contêiner DEF que o
# kernel carrega (é o único formato de userspace aceito). O build de userland
# depende dele, então esta etapa precisa estar pronta antes.
.PHONY: tools dnu-pack dnulink

tools: $(DFS_IMAGE_BIN) $(DNU_PACK_BIN) $(DNULINK_BIN)

dnu-pack: $(DNU_PACK_BIN)

# O dnulink é o `linker` do target x86_64-unknown-dreamcore: recebe o ELF do
# rust-lld e publica DEF. `make rust-target` instala este binário em
# `~/.cargo/dreamcore/` para o `cargo build` do usuário encontrá-lo.
dnulink: $(DNULINK_BIN)

$(DFS_IMAGE_BIN): $(ROOT)/common/tools/dfs-image.rs $(ROOT)/dnu_kernel/src/fs/dfs/dfs.rs \
                  $(ROOT)/dnu_kernel/src/fs/gpt.rs $(ROOT)/dnu_kernel/src/drivers/block.rs
	@mkdir -p $(BUILD_DIR)
	$(RUSTC) --edition 2021 $< -o $@

$(ABI_RLIB): $(ABI_MANIFEST) $(wildcard $(ROOT)/common/shared/abi/src/*.rs)
	$(CARGO) build --manifest-path $(ABI_MANIFEST) --release \
		--target $(HOST_TRIPLE) --target-dir $(ABI_TARGET_DIR)

$(DNU_PACK_BIN): $(ROOT)/common/tools/dnu-pack.rs $(ABI_RLIB)
	@mkdir -p $(BUILD_DIR)
	$(RUSTC) --edition 2024 -C panic=abort \
		--extern imagineos_abi=$(ABI_RLIB) $< -o $@

$(DNULINK_BIN): $(ROOT)/common/tools/dnulink.rs $(ABI_RLIB)
	@mkdir -p $(BUILD_DIR)
	$(RUSTC) --edition 2024 -C panic=abort \
		--extern imagineos_abi=$(ABI_RLIB) $< -o $@

# ------------------------------------------------------------------------------
# Alvo Rust x86_64-unknown-dreamcore
# ------------------------------------------------------------------------------
# Constrói o programa de exemplo do alvo. Precisa de `rust-src` no nightly
# (`-Z build-std` recompila `core`/`alloc` para o alvo).
RUST_TARGET    := $(ROOT)/targets/x86_64-unknown-dreamcore.json
RUST_EXAMPLE   := $(ROOT)/examples/rust-hello

.PHONY: rust-target rust-example rust-std rust-std-patch rust-std-new

# Instala o `dnulink` onde o cargo do usuário o encontra e verifica que o
# target JSON é aceito pelo rustc.
rust-target: $(DNULINK_BIN) $(RUST_TARGET)
	@mkdir -p $(HOME)/.cargo/dreamcore
	@cp $(DNULINK_BIN) $(HOME)/.cargo/dreamcore/dnulink
	@rustc +nightly --target $(RUST_TARGET) --print target-spec-json \
		-Z unstable-options >/dev/null && echo "alvo x86_64-unknown-dreamcore ok"

rust-example: rust-target
	cd $(RUST_EXAMPLE) && \
		RUSTC_BOOTSTRAP=1 rustup run nightly cargo build --release -Zjson-target-spec && \
		echo "DEF: $(RUST_EXAMPLE)/target/x86_64-unknown-dreamcore/release/rust-hello"

# Aplica a camada `dreamcore` na std do toolchain (116 linhas de patch +
# `os/unix/dreamcore`). Use --revert no script para restaurar.
RUST_STD_EXAMPLE := $(ROOT)/examples/rust-std-hello

rust-std-patch:
	$(ROOT)/std-dreamcore/patches/apply.sh

rust-std: rust-target
	$(ROOT)/std-dreamcore/patches/apply.sh
	cd $(RUST_STD_EXAMPLE) && \
		RUSTC_BOOTSTRAP=1 rustup run nightly cargo build --release -Zjson-target-spec -Zbuild-std=std && \
		echo "DEF (std): $(RUST_STD_EXAMPLE)/target/x86_64-unknown-dreamcore/release/rust-std-hello"

# Cria um projeto novo já configurado: `make rust-std-new DIR=~/meu-app`.
DIR ?=
rust-std-new: rust-target
	@if [ -z "$(DIR)" ]; then echo "uso: make rust-std-new DIR=<diretório>"; exit 2; fi
	$(ROOT)/std-dreamcore/new-project.sh "$(DIR)"
