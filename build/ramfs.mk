# ==============================================================================
# ImagineOS - estrutura e conteúdo da raiz volátil (estilo Linux/FHS)
# ==============================================================================
#
# A árvore `ramfs/` da raiz deste repositório é o esqueleto do sistema; ela é
# entregue ao repositório DNU (`RAMFS_DIR`) e entra no initramfs **dentro** do
# dzImage, ao lado do kernel. O ImagineOS não empacota mais um
# `initramfs.tar`/`initramfs-installed.tar` próprio: o contêiner é a única
# unidade de boot.
#
# Os artefatos do userland (programas, libc, crt, linker script) são montados
# em `.build/ramfs/` e copiados para a árvore antes de o DNU empacotar:
#   * a árvore de dados de `ramfs/` com a fonte PSF copiada;
#   * os headers da libc e as libs estáticas (libdreamcore.a, libc.a, libm.a);
#   * os objetos crt0/crt1/crti/crtn e o linker script do userspace;
#   * os binários compilados (init, getty, shell e utilitários);
#   * o payload do instalador (dzImage, Limine) em system/install/.
#
# O `dzImage` instalado como payload é o mesmo que o DNU publica; o boot da
# ISO carrega `boot/dzImage` + `boot/initramfs.tar`, e o instalador copia o
# dzImage para o disco (ver `boot/startup.nsh` e `build/disk.mk`).
#
# Uso isolado:  make -f build/ramfs.mk
# Alvos:        ramfs, ramfs-tree, ramfs-image
RAMFS_MK := $(abspath $(lastword $(MAKEFILE_LIST)))

include $(dir $(lastword $(MAKEFILE_LIST)))common.mk
# As peças que entram na árvore vêm de outros makefiles. Em vez de duplicar
# suas regras aqui, pedimos que o make os construa recursivamente.
KERNEL_MK   := $(ROOT)/build/kernel.mk
USERLAND_MK := $(ROOT)/build/userland.mk
TOOLS_MK    := $(ROOT)/build/tools.mk

# Todo arquivo da árvore `ramfs/` é dependência da árvore montada: editar um
# script em `etc/` (ou trocar uma fonte) precisa refletir no dzImage.
RAMFS_SOURCES := $(shell find $(RAMFS_ROOT) -type f -o -type l 2>/dev/null)

# Árvore completa (dados + userland) que o DNU empacota; `RAMFS_TREE` vem de
# common.mk, o mesmo caminho que kernel.mk passa ao DNU.

.PHONY: ramfs ramfs-tree ramfs-image ramfs-inputs

ramfs:
	@mkdir -p $(addprefix $(RAMFS_ROOT)/,$(RAMFS_DIRS))

# Constrói tudo o que entra na árvore antes de montá-la. O dzImage só é
# montado em `ramfs-image`/`iso`, depois desta árvore: a regra do dzImage lista
# `ramfs/` como dependência, então pedi-lo aqui criaria um ciclo.
ramfs-inputs: ramfs
	$(MAKE) -f $(TOOLS_MK) tools
	$(MAKE) -f $(USERLAND_MK) user-programs user-libs \
		$(BUILD_DIR)/user/utilities/hello_c

# `ramfs-image` mantém o nome histórico: agora monta a árvore e deixa o DNU
# empacotá-la no dzImage.
ramfs-image: ramfs-tree
	$(MAKE) -f $(KERNEL_MK) dzimage

ramfs-tree: ramfs-inputs $(RAMFS_SOURCES)
	@test -f $(BUILD_DIR)/user/sbin/init || (echo "programas de userspace ausentes"; exit 1)
	rm -rf $(RAMFS_TREE)
	mkdir -p $(RAMFS_TREE)
	cp -a $(RAMFS_ROOT)/. $(RAMFS_TREE)/
	mkdir -p $(RAMFS_TREE)/system/fonts
	cp $(DNU_DIR)/src/assets/zap-vga16.psf $(RAMFS_TREE)/system/fonts/zap-vga16.psf
	mkdir -p $(RAMFS_TREE)/usr/include $(RAMFS_TREE)/usr/lib/tcc
	cp -a $(USER_LIBC_DIR)/include/. $(RAMFS_TREE)/usr/include/
	cp $(BUILD_DIR)/user/libdreamcore.a $(RAMFS_TREE)/usr/lib/libdreamcore.a
	cp $(BUILD_DIR)/user/libdreamcore.a $(RAMFS_TREE)/usr/lib/libc.a
	$(AR) rcs $(BUILD_DIR)/user/libm.a $(BUILD_DIR)/user/math.o
	cp $(BUILD_DIR)/user/libm.a $(RAMFS_TREE)/usr/lib/libm.a
	cp $(BUILD_DIR)/user/crt.o $(RAMFS_TREE)/usr/lib/crt0.o
	cp $(BUILD_DIR)/user/crt.o $(RAMFS_TREE)/usr/lib/crt1.o
	cp $(BUILD_DIR)/user/crt_empty.o $(RAMFS_TREE)/usr/lib/crti.o
	cp $(BUILD_DIR)/user/crt_empty.o $(RAMFS_TREE)/usr/lib/crtn.o
	cp $(ROOT)/common/userland/linker.ld $(RAMFS_TREE)/usr/lib/imagineos.ld
	# Binários userland
	cp $(BUILD_DIR)/user/sbin/init $(RAMFS_TREE)/sbin/init
	cp $(BUILD_DIR)/user/sbin/getty $(RAMFS_TREE)/sbin/getty
	cp $(BUILD_DIR)/user/bin/shell $(RAMFS_TREE)/bin/shell
	for utility in $(USER_UTILITIES) $(USER_C_UTILITIES); do \
		cp $(BUILD_DIR)/user/utilities/$$utility $(RAMFS_TREE)/bin/$$utility; \
	done
	# Programas Rust do alvo x86_64-unknown-dreamcore. São opcionais: se a
	# ferramenta não foi construída, a imagem segue sem eles.
	for program in rust-hello rust-std-hello; do \
		binary=$(ROOT)/examples/$$program/target/x86_64-unknown-dreamcore/release/$$program; \
		if [ -f "$$binary" ]; then cp "$$binary" $(RAMFS_TREE)/bin/$$program; fi; \
	done
	# Stack Liminal: display server + window manager/compositor.
	cp $(BUILD_DIR)/user/bin/liminal-server $(RAMFS_TREE)/bin/liminal-server
	cp $(BUILD_DIR)/user/bin/liminal-client $(RAMFS_TREE)/bin/liminal-client

	# Temas (.tpf — Theme Package File): empacota o tema clássico num TAR ustar
	# com `themeConfig.json` (nome, autor, versão, descrição, compositor) e o
	# CSS `window/windowTheme.css`. O WM olha para system/themes/current.tpf,
	# com fallback classic.tpf; as fontes do tema ficam em ramfs/system/themes/classic.
	# Sem as fontes do tema, o WM usa o tema embutido — não é erro de build.
	mkdir -p $(RAMFS_TREE)/system/themes
	if [ -f $(RAMFS_ROOT)/system/themes/classic/themeConfig.json ]; then \
		tar --format=ustar -C $(RAMFS_ROOT)/system/themes/classic \
			--exclude='*cursor*' -cf $(RAMFS_TREE)/system/themes/classic.tpf themeConfig.json window; \
		cp $(RAMFS_TREE)/system/themes/classic.tpf $(RAMFS_TREE)/system/themes/current.tpf; \
	else \
		echo "temas: fontes em ramfs/system/themes/classic ausentes; WM usara o tema embutido"; \
	fi

	# Payload do instalador: o próprio dzImage e os arquivos do Limine chegam da
	# ISO (`iso.mk`); aqui só garantimos o diretório.
	mkdir -p $(RAMFS_TREE)/system/install
