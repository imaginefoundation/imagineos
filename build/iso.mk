# ==============================================================================
# ImagineOS - ISO inicializável e execução no QEMU (layout estilo Linux/FHS)
# ==============================================================================
#
# Monta a ISO UEFI com a organização de um live CD Linux:
#
#   /boot/dzImage          DNU Zipped Image — kernel + initramfs num arquivo
#   /EFI/BOOT/BOOTX64.EFI  Limine (bootloader EFI, registro El Torito)
#
# O `dzImage` é o único artefato de boot: o Limine o carrega como módulo e o
# kernel extrai o `initramfs` da tabela de membros (ver `dnu/boot/boot.rs`).
#
# O xorriso produz ISO 9660 com Rock Ridge (-R), Joliet (-J) e El Torito
# (--efi-boot-part), exatamente o combo que distribuições Linux usam.
#
# Uso isolado:  make -f build/iso.mk iso
# Alvos:        iso, run, disk-image, run-disk, installer-disk, run-installer

include $(dir $(lastword $(MAKEFILE_LIST)))common.mk

# Cada etapa tem seu próprio makefile, então a ISO depende dos *alvos* das
# outras etapas (que o make resolve recursivamente) em vez de tentar recriar
# as regras de arquivo que vivem em kernel.mk/ramfs.mk.
KERNEL_MK := $(ROOT)/build/kernel.mk
TOOLS_MK  := $(ROOT)/build/tools.mk
RAMFS_MK  := $(ROOT)/build/ramfs.mk

.PHONY: iso run disk-image run-disk installer-disk run-installer \
        image-payloads

# Payloads da ISO: o dzImage já contém a raiz volátil (montada por ramfs.mk).
image-payloads:
	$(MAKE) -f $(RAMFS_MK) ramfs-tree
	$(MAKE) -f $(KERNEL_MK) dzimage

# ------------------------------------------------------------------------------
# ISO
# ------------------------------------------------------------------------------

iso: image-payloads
	@# Confirma que os artefatos existem antes de montar a ISO.
	@test -f $(DZ_IMAGE) || (echo "dzImage ausente"; exit 1)
	@command -v xorriso >/dev/null || (echo "xorriso não encontrado"; exit 1)
	@command -v mkfs.vfat >/dev/null || (echo "mkfs.vfat não encontrado"; exit 1)
	@command -v mcopy >/dev/null || (echo "mcopy não encontrado"; exit 1)
	@command -v mmd >/dev/null || (echo "mmd não encontrado"; exit 1)
	rm -rf $(ISO_DIR)
	mkdir -p $(ISO_DIR)/EFI/BOOT $(ISO_DIR)/boot $(DISTRO_DIR)
	cp $(DZ_IMAGE) $(ISO_DIR)/boot/dzImage
	cp $(ROOT)/boot/limine.conf $(ISO_DIR)/limine.conf
	cp $(LIMINE_DIR)/BOOTX64.EFI $(ISO_DIR)/EFI/BOOT/BOOTX64.EFI
	@# Dimensiona a imagem FAT do ESP com folga para os payloads de boot
	@# (BOOTX64.EFI + dzImage), em vez de um tamanho fixo.
	@payload_bytes=$$( cat $(ISO_DIR)/EFI/BOOT/BOOTX64.EFI $(ISO_DIR)/boot/dzImage 2>/dev/null | wc -c ); \
		blocks=$$(( (payload_bytes + 1024*1024 - 1) / (1024*1024) + 16 )); \
		echo "Tamanho da imagem EFI: $${blocks} MiB (payload: $${payload_bytes} bytes)"; \
		dd if=/dev/zero of=$(ISO_DIR)/efi.img bs=1M count=$${blocks}
	mkfs.vfat $(ISO_DIR)/efi.img
	mmd -i $(ISO_DIR)/efi.img ::/EFI ::/EFI/BOOT ::/boot
	mcopy -i $(ISO_DIR)/efi.img $(ISO_DIR)/EFI/BOOT/BOOTX64.EFI ::/EFI/BOOT/
	mcopy -i $(ISO_DIR)/efi.img $(ISO_DIR)/limine.conf ::/limine.conf
	mcopy -i $(ISO_DIR)/efi.img $(ISO_DIR)/boot/dzImage ::/boot/dzImage
	xorriso -as mkisofs -R -r -J -V DREAMCORE \
		--efi-boot efi.img -efi-boot-part --efi-boot-image \
		--protective-msdos-label $(ISO_DIR) -o $(ISO_IMAGE)
	@printf 'ISO gerada com sucesso: %s\n' '$(ISO_IMAGE)'

# ------------------------------------------------------------------------------
# Execução no QEMU
# ------------------------------------------------------------------------------

run: iso
	@test -f "$(OVMF_CODE)" || (echo "OVMF firmware não encontrado em $(OVMF_CODE)"; exit 1)
	$(QEMU) -machine q35 -m 512M -serial stdio \
		-drive if=pflash,format=raw,unit=0,readonly=on,file="$(OVMF_CODE)" \
		-cdrom $(ISO_IMAGE)

# A imagem de disco é semeada com a raiz do próprio dzImage (o `dfs-image`
# extrai o membro `initramfs`), sem um initramfs paralelo.
disk-image: iso
	$(MAKE) -f $(TOOLS_MK) tools
	@if [ -e "$(DISK_IMAGE)" ]; then \
		printf 'Usando imagem de disco existente: %s\n' "$(DISK_IMAGE)"; \
	else \
		sh $(DNU_DIR)/scripts/install-disk.sh "$(DISK_IMAGE)" \
			"$(ISO_DIR)/boot/dzImage" \
			"$(ISO_DIR)/EFI/BOOT/BOOTX64.EFI" "$(ISO_DIR)/limine.conf" \
			"$(ROOT)/boot/startup.nsh" "$(DFS_IMAGE_BIN)"; \
	fi

run-disk: disk-image
	@test -f "$(OVMF_CODE)" || (echo "OVMF firmware não encontrado em $(OVMF_CODE)"; exit 1)
	$(QEMU) -machine pc -m 512M -serial stdio \
		-drive if=pflash,format=raw,unit=0,readonly=on,file="$(OVMF_CODE)" \
		-drive if=ide,index=0,format=raw,file="$(DISK_IMAGE)"

installer-disk:
	@mkdir -p $(BUILD_DIR)
	@if [ ! -e "$(INSTALL_TARGET)" ]; then \
		qemu-img create -f raw "$(INSTALL_TARGET)" 1G; \
	fi

run-installer: iso installer-disk
	@test -f "$(OVMF_CODE)" || (echo "OVMF firmware não encontrado em $(OVMF_CODE)"; exit 1)
	$(QEMU) -machine pc -m 512M -serial stdio -boot order=d \
		-drive if=pflash,format=raw,unit=0,readonly=on,file="$(OVMF_CODE)" \
		-drive if=ide,index=0,format=raw,file="$(INSTALL_TARGET)" \
		-cdrom $(ISO_IMAGE)
