# ==============================================================================
# ImagineOS - kernel e dzImage
# ==============================================================================
#
# O kernel `dreamcore` e o formato DEF são propriedade do repositório DNU
# (`$(DNU_DIR)`), que publica **um único artefato**: `.build/dzImage` (DNU
# Zipped Image), o contêiner com o ELF do kernel e o initramfs. Toda a
# estrutura do ImagineOS (ISO, imagem de disco, diretório do instalador) consome
# só esse arquivo e nunca o ELF cru.
#
# O dzImage é montado pelo `Makefile` do DNU, com `RAMFS_DIR=` apontando para a
# árvore montada por `ramfs.mk` (`.build/ramfs`: esqueleto `ramfs/` + userland).
# Este makefile não compila nada do kernel.
#
# Uso isolado:  make -f build/kernel.mk
# Alvos:        kernel, dzimage
include $(dir $(lastword $(MAKEFILE_LIST)))common.mk
.PHONY: kernel dzimage

# ------------------------------------------------------------------------------
# Kernel e dzImage (delegados ao repositório DNU)
# ------------------------------------------------------------------------------

# `kernel` continua existindo apenas para compatibilidade: extrai o ELF do
# contêiner já montado (quem carrega o kernel é o Limine, a partir do dzImage).
kernel: $(DZ_IMAGE)
	@mkdir -p $(BUILD_DIR)
	$(DFS_IMAGE_BIN) --dump-kernel $(DZ_IMAGE) $(KERNEL)

dzimage: $(DZ_IMAGE)

# A árvore que vira a raiz volátil é construída por ramfs.mk e o dzImage é
# montado pelo DNU por cima dela (o kernel e o initramfs são membros do mesmo
# contêiner).
$(DZ_IMAGE): $(DNU_MK) $(wildcard $(DNU_DIR)/src/*.rs) \
             $(wildcard $(DNU_DIR)/src/assets/*) \
             $(wildcard $(DNU_DIR)/abi/imagineos_abi/src/*.rs) \
             $(DNU_DIR)/dzimage.rs $(DNU_DIR)/tools/abi.rs \
             $(wildcard $(BUILD_DIR)/ramfs/*)
	@test -d $(DNU_DIR) || { echo "repositório DNU ausente em $(DNU_DIR)"; exit 1; }
	$(MAKE) -C $(DNU_DIR) dzImage RAMFS_DIR=$(RAMFS_TREE)
	@mkdir -p $(BUILD_DIR)
	cp $(DNU_DIR)/.build/dzImage $(DZ_IMAGE)
	$(DNU_DIR)/.build/dzimage info $(DZ_IMAGE)
