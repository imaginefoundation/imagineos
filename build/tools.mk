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
.PHONY: tools dnu-pack

tools: $(DFS_IMAGE_BIN) $(DNU_PACK_BIN)

dnu-pack: $(DNU_PACK_BIN)

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
