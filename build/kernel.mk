# ==============================================================================
# ImagineOS - kernel e dzImage
# ==============================================================================
#
# Compila o kernel `dreamcore` e publica o ELF como `.build/dzImage`. O Limine
# carrega esse ELF diretamente na RAM (protocolo `limine`), não há bootstrap
# de segunda fase nem payload comprimido.
#
# Uso isolado:  make -f build/kernel.mk
# Alvos:        kernel, dzimage, tools
include $(dir $(lastword $(MAKEFILE_LIST)))common.mk
.PHONY: kernel dzimage tools

# ------------------------------------------------------------------------------
# Kernel
# ------------------------------------------------------------------------------

kernel: $(KERNEL)

$(KERNEL): $(wildcard $(ROOT)/kernel/src/**/*.rs) $(ROOT)/kernel/linker.ld $(FONT_ASSET)
	$(CARGO) build --release --target $(TARGET) --bin dreamcore \
		--manifest-path $(ROOT)/Cargo.toml \
		--config 'target.x86_64-unknown-none.rustflags=["-C","link-arg=-Tkernel/linker.ld","-C","relocation-model=static"]' \
		$(if $(KERNEL_FEATURES),--features $(KERNEL_FEATURES),)

# ------------------------------------------------------------------------------
# Ferramentas do host usadas pelo kernel/ISO
# ------------------------------------------------------------------------------
tools: $(DFS_IMAGE_BIN)
$(DFS_IMAGE_BIN): $(ROOT)/tools/dfs-image.rs $(ROOT)/kernel/src/fs/dfs.rs \
                  $(ROOT)/kernel/src/fs/gpt.rs $(ROOT)/kernel/src/drivers/block.rs
	@mkdir -p $(BUILD_DIR)
	$(RUSTC) --edition 2021 $< -o $@

# ------------------------------------------------------------------------------
# dzImage (ELF do kernel, carregado diretamente pelo Limine)
# ------------------------------------------------------------------------------
dzimage: $(DZ_IMAGE)
$(DZ_IMAGE): $(KERNEL)
	@mkdir -p $(BUILD_DIR)
	cp $(KERNEL) $@
