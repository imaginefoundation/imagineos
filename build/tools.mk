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

.PHONY: tools

tools: $(DFS_IMAGE_BIN)

$(DFS_IMAGE_BIN): $(ROOT)/tools/dfs-image.rs $(ROOT)/kernel/src/fs/dfs.rs \
                  $(ROOT)/kernel/src/fs/gpt.rs $(ROOT)/kernel/src/drivers/block.rs
	@mkdir -p $(BUILD_DIR)
	$(RUSTC) --edition 2021 $< -o $@
