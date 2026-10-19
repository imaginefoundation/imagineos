# ==============================================================================
# ImagineOS - variáveis compartilhadas entre os makefiles de build
# ==============================================================================
#
# Cada makefile em build/ inclui este arquivo. O Makefile raiz também o inclui,
# então os nomes de diretórios e ferramentas existem em um único lugar e os
# sub-makefiles podem ser executados tanto pela raiz quanto isoladamente:
#
#   make -f build/kernel.mk          # compila kernel + dzImage
#   make -f build/tools.mk           # compila as ferramentas do host
#   make -f build/userland.mk        # compila apps e utilitários
#   make -f build/ramfs.mk           # empacota o RAMFS
#   make -f build/iso.mk             # gera a ISO
#
# A raiz orquestra chamando estes mesmos arquivos com $(MAKE) -f.
SHELL := /bin/sh

ROOT := $(patsubst %/,%,$(dir $(lastword $(MAKEFILE_LIST))))/..
ROOT := $(abspath $(ROOT))

# ==============================================================================
# IDENTIDADE E ALVO
# ==============================================================================
K_NAME         := dreamcore
K_VERSION      := 0.1.0
CODENAME       := astrid
DISTRIBUTION   := imagineos
TARGET         := x86_64-unknown-none
KERNEL_FEATURES ?=

# ==============================================================================
# FERRAMENTAS
# ==============================================================================
QEMU           := qemu-system-x86_64
OVMF_CODE      ?= OVMF.fd
CARGO          := rustup run stable cargo
RUSTC          := rustup run stable rustc
CC             ?= cc
AR             ?= ar

# ==============================================================================
# DIRETÓRIOS E ARTEFATOS DE SAÍDA
# ==============================================================================
BUILD_DIR      := $(ROOT)/.build
DISTRO_DIR     := $(BUILD_DIR)/distro
KERNEL         := $(ROOT)/target/$(TARGET)/release/imagineos-dnu-kernel
DZ_IMAGE       := $(BUILD_DIR)/dzImage
RAMFS_IMAGE    := $(BUILD_DIR)/initramfs.tar
RAMFS_INST_IMG := $(BUILD_DIR)/initramfs-installed.tar
ISO_DIR        := $(BUILD_DIR)/iso
ISO_IMAGE      := $(DISTRO_DIR)/$(K_NAME)-$(shell date +%Y-%m-%d-%H-%M)-$(CODENAME).iso
DISK_IMAGE     ?= $(BUILD_DIR)/imagineos-disk.img
INSTALL_TARGET := $(BUILD_DIR)/installer-target.img
DFS_IMAGE_BIN  := $(BUILD_DIR)/dfs-image
DNU_PACK_BIN   := $(BUILD_DIR)/dnu-pack

# Fontes de terceiros e recursos
LIMINE_DIR      := $(ROOT)/third_party/limine
FONT_ASSET      := $(ROOT)/common/tools/assets/zap-vga16.psf

# ==============================================================================
# RAMFS E PROGRAMAS DE USUÁRIO
# ==============================================================================
RAMFS_ROOT     := $(ROOT)/ramfs
RAMFS_DIRS     := bin dev etc sbin home system/fonts system/install tmp usr

# Programas de usuário: os fontes vivem em common/ (programs/ e utilities/).
USER_UTILITIES   := cat dmesg distroinstall fdtest forktest globalconf grep kill ls mkdir ps rm shutdown su sudo touch uname vi hello systemfetch top
USER_C_UTILITIES := hello_c

# Cabeçalhos e fontes da libc de userspace (common/userland/libraries/libc).
USER_LIBC_DIR   := $(ROOT)/common/userland/libraries/libc
USER_LIBC_INC   := $(USER_LIBC_DIR)/include

# Manifestos e libs do userland
USER_API_MANIFEST       := $(ROOT)/common/userland/api/Cargo.toml
USER_API_TARGET_DIR     := $(BUILD_DIR)/user/api-target
USER_API_RLIB           := $(USER_API_TARGET_DIR)/$(TARGET)/release/libimagineos.rlib
USER_API_DEPS           := $(USER_API_TARGET_DIR)/$(TARGET)/release/deps
USER_RUNTIME_MANIFEST   := $(ROOT)/common/userland/runtimes/ImagineRustRuntime/Cargo.toml
USER_RUNTIME_TARGET_DIR := $(BUILD_DIR)/user/runtime-target
USER_RUNTIME_RLIB       := $(USER_RUNTIME_TARGET_DIR)/$(TARGET)/release/libimagineos_rt.rlib
USER_RUNTIME_DEPS       := $(USER_RUNTIME_TARGET_DIR)/$(TARGET)/release/deps

# Runtime de programas em C, implementada em Rust
# (common/userland/runtimes/ImagineCRuntime).
# Produz um único objeto com _start, c_runtime_init, setjmp/longjmp e o
# restaurador de sinais, substituindo os antigos crt0.S/crt0.c/signal.S/setjmp.S.
USER_CRT_MANIFEST       := $(ROOT)/common/userland/runtimes/ImagineCRuntime/Cargo.toml
USER_CRT_TARGET_DIR     := $(BUILD_DIR)/user/crt-target
USER_CRT_RLIB           := $(BUILD_DIR)/user/libimagineos_crt.a
USER_PROGRAMS           := $(BUILD_DIR)/user/sbin/init \
                           $(BUILD_DIR)/user/sbin/getty \
                           $(BUILD_DIR)/user/bin/shell \
                           $(BUILD_DIR)/user/bin/liminal-client \
                           $(BUILD_DIR)/user/bin/liminal-server \
                           $(addprefix $(BUILD_DIR)/user/utilities/,$(USER_UTILITIES) $(USER_C_UTILITIES))
# ELFs intermediários: o toolchain só emite ELF estático e o `dnu-pack` os
# converte para DEF (o formato que o kernel carrega). Ficam no build, não no RAMFS.
USER_PROGRAM_ELFS       := $(addsuffix .elf,$(USER_PROGRAMS))
USER_CFLAGS             := -ffreestanding -fno-stack-protector -fno-pic -fno-pie \
                           -fno-builtin -mno-red-zone -nostdinc -I$(USER_LIBC_INC) \
                           -isystem $(shell $(CC) -print-file-name=include)
USER_CRT_OBJECTS        := $(BUILD_DIR)/user/crt.o \
                           $(BUILD_DIR)/user/unistd.o \
                           $(BUILD_DIR)/user/path.o $(BUILD_DIR)/user/string.o $(BUILD_DIR)/user/stdio.o \
                           $(BUILD_DIR)/user/stdlib.o $(BUILD_DIR)/user/time.o \
                           $(BUILD_DIR)/user/math.o $(BUILD_DIR)/user/mman.o \
                           $(BUILD_DIR)/user/signals.o
USER_LIBC_OBJECTS       := $(filter-out $(BUILD_DIR)/user/crt.o,$(USER_CRT_OBJECTS))

# ------------------------------------------------------------------------------
# musl portada para o ImagineOS (third_party/musl-1.2.6)
# ------------------------------------------------------------------------------
# A musl é compilada pelo `configure`/`make` dela, com `syscall_arch.h`
# reescrito para `int 0x80` e `bits/syscall.h.in` com os números do Dreamcore.
# O build do projeto não roda o `configure` a cada compilação: o diretório
# `third_party/musl-1.2.6` já vem configurado para este alvo.
MUSL_DIR         := $(ROOT)/third_party/musl-1.2.6
MUSL_LIBC        := $(MUSL_DIR)/lib/libc.a
MUSL_INC         := $(MUSL_DIR)/obj/include
MUSL_STAMP       := $(BUILD_DIR)/musl.stamp

$(MUSL_STAMP): $(wildcard $(MUSL_DIR)/src/**/*.c) $(wildcard $(MUSL_DIR)/src/**/*.s) \
               $(wildcard $(MUSL_DIR)/arch/x86_64/*.h)
	@mkdir -p $(BUILD_DIR)
	$(MAKE) -C $(MUSL_DIR) -j$$(nproc)
	@touch $@

musl: $(MUSL_STAMP)
	@echo "musl pronta: $(MUSL_LIBC)"

.PHONY: musl
