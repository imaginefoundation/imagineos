# ==============================================================================
# ImagineOS - programas de usuário (Rust, C e libc)
# ==============================================================================
#
# Compila os programas de userspace para o alvo x86_64-unknown-none:
#   * apps principais: init, getty, shell   -> .build/user/{sbin,bin}
#   * stack Liminal: liminal-server, liminal-client -> .build/user/bin
#   * utilitários Rust: ls, cat, ...                -> .build/user/utilities
#   * utilitários C: hello_c                        -> .build/user/utilities
#   * libc estática (libdreamcore.a), objetos crt e runtime C.
#
# Os fontes vivem em common/: programas multi-módulo em common/programs/ e
# utilitários de arquivo único em common/utilities/. A libc fica em
# userland/libraries/libc/.
#
# Uso isolado:  make -f build/userland.mk
# Alvos:        user-programs, user-libs, c-app
include $(dir $(lastword $(MAKEFILE_LIST)))common.mk

PROGRAMS_DIR  := $(ROOT)/common/programs
UTILITIES_DIR := $(ROOT)/common/utilities

.PHONY: user-programs user-libs c-app

user-programs: $(USER_PROGRAMS)

user-libs: $(BUILD_DIR)/user/libdreamcore.a $(BUILD_DIR)/user/crt.o $(BUILD_DIR)/user/crt_empty.o

# ------------------------------------------------------------------------------
# Libs de userspace (crate API e runtime Rust)
# ------------------------------------------------------------------------------
$(USER_API_RLIB): $(USER_API_MANIFEST) $(wildcard $(ROOT)/userland/api/src/*.rs) \
                  $(ROOT)/shared/abi/Cargo.toml $(ROOT)/shared/abi/src/lib.rs
	$(CARGO) build --manifest-path $(USER_API_MANIFEST) \
		--target $(TARGET) --release --target-dir $(USER_API_TARGET_DIR)

$(USER_RUNTIME_RLIB): $(USER_RUNTIME_MANIFEST) $(ROOT)/userland/runtimes/ImagineRustRuntime/src/lib.rs $(USER_API_RLIB)
	$(CARGO) build --manifest-path $(USER_RUNTIME_MANIFEST) \
		--target $(TARGET) --release --target-dir $(USER_RUNTIME_TARGET_DIR)

$(USER_CRT_RLIB): $(USER_CRT_MANIFEST) $(ROOT)/userland/runtimes/ImagineCRuntime/src/lib.rs $(USER_API_RLIB)
	@mkdir -p $(BUILD_DIR)/user
	$(CARGO) build --manifest-path $(USER_CRT_MANIFEST) \
		--target $(TARGET) --release --target-dir $(USER_CRT_TARGET_DIR)
	cp $(USER_CRT_TARGET_DIR)/$(TARGET)/release/libimagineos_crt.a $@

# ------------------------------------------------------------------------------
# Programas Rust de userspace
# ------------------------------------------------------------------------------

# Apps de arquivo único em common/programs/<nome>.rs (init, getty).
$(BUILD_DIR)/user/sbin/%: $(PROGRAMS_DIR)/%.rs $(ROOT)/userland/linker.ld \
                          $(USER_API_RLIB) $(USER_RUNTIME_RLIB)
	@mkdir -p $(dir $@)
	$(RUSTC) --crate-name $* --edition 2024 --target $(TARGET) \
		--extern imagineos=$(USER_API_RLIB) --extern imagineos_rt=$(USER_RUNTIME_RLIB) \
		-L dependency=$(USER_API_DEPS) -L dependency=$(USER_RUNTIME_DEPS) \
		-C panic=abort -C relocation-model=static \
		-C link-arg=-T$(ROOT)/userland/linker.ld $< -o $@

# O shell é um binário multi-módulo (`common/programs/shell/`), então tem
# regra própria: o padrão `%.rs` não cobre `main.rs` mais os módulos irmãos.
SHELL_SOURCES := $(wildcard $(PROGRAMS_DIR)/shell/*.rs)

$(BUILD_DIR)/user/bin/shell: $(filter-out %/tests.rs,$(SHELL_SOURCES)) $(ROOT)/userland/linker.ld \
                             $(USER_API_RLIB) $(USER_RUNTIME_RLIB)
	@mkdir -p $(dir $@)
	$(RUSTC) --crate-name shell --edition 2024 --target $(TARGET) \
	--extern imagineos=$(USER_API_RLIB) --extern imagineos_rt=$(USER_RUNTIME_RLIB) \
	-L dependency=$(USER_API_DEPS) -L dependency=$(USER_RUNTIME_DEPS) \
	-C panic=abort -C relocation-model=static \
	-C link-arg=-T$(ROOT)/userland/linker.ld \
	$(PROGRAMS_DIR)/shell/main.rs -o $@

# Horust (init-style): multi-módulo em common/programs/horust/, instalado
# em /sbin/horust. O horust-lite.rs raiz é o binário; os módulos ficam ao lado.
HORUST_SOURCES := $(wildcard $(PROGRAMS_DIR)/horust/*.rs)

$(BUILD_DIR)/user/sbin/horust: $(filter-out %/tests.rs,$(HORUST_SOURCES)) \
                               $(PROGRAMS_DIR)/horust-lite.rs \
                               $(ROOT)/userland/linker.ld $(USER_API_RLIB) $(USER_RUNTIME_RLIB)
	@mkdir -p $(dir $@)
	$(RUSTC) --crate-name horust --edition 2024 --target $(TARGET) \
	--extern imagineos=$(USER_API_RLIB) --extern imagineos_rt=$(USER_RUNTIME_RLIB) \
	-L dependency=$(USER_API_DEPS) -L dependency=$(USER_RUNTIME_DEPS) \
	-C panic=abort -C relocation-model=static \
	-C link-arg=-T$(ROOT)/userland/linker.ld \
	$(PROGRAMS_DIR)/horust-lite.rs -o $@

# Liminal stack: o liminalServer é o display server (framebuffer, composição
# em backbuffer, eventos); o liminalClient é o window manager/compositor
# (janelas, temas .tpf, apps .xapp).
LIMINAL_SERVER_SOURCES := $(wildcard $(PROGRAMS_DIR)/liminalServer/*.rs)

$(BUILD_DIR)/user/bin/liminal-server: $(LIMINAL_SERVER_SOURCES) $(ROOT)/userland/linker.ld \
                                     $(USER_API_RLIB) $(USER_RUNTIME_RLIB)
	@mkdir -p $(dir $@)
	$(RUSTC) --crate-name liminal_server --edition 2024 --target $(TARGET) \
	--extern imagineos=$(USER_API_RLIB) --extern imagineos_rt=$(USER_RUNTIME_RLIB) \
	-L dependency=$(USER_API_DEPS) -L dependency=$(USER_RUNTIME_DEPS) \
	-C panic=abort -C relocation-model=static \
	-C link-arg=-T$(ROOT)/userland/linker.ld \
	$(PROGRAMS_DIR)/liminalServer/main.rs -o $@

LIMINAL_CLIENT_SOURCES := $(wildcard $(PROGRAMS_DIR)/liminalClient/*.rs)

$(BUILD_DIR)/user/bin/liminal-client: $(LIMINAL_CLIENT_SOURCES) $(ROOT)/userland/linker.ld \
                                     $(USER_API_RLIB) $(USER_RUNTIME_RLIB)
	@mkdir -p $(dir $@)
	$(RUSTC) --crate-name liminal_client --edition 2024 --target $(TARGET) \
	--extern imagineos=$(USER_API_RLIB) --extern imagineos_rt=$(USER_RUNTIME_RLIB) \
	-L dependency=$(USER_API_DEPS) -L dependency=$(USER_RUNTIME_DEPS) \
	-C panic=abort -C relocation-model=static \
	-C link-arg=-T$(ROOT)/userland/linker.ld \
	$(PROGRAMS_DIR)/liminalClient/main.rs -o $@

# ------------------------------------------------------------------------------
# Utilitários Rust (common/utilities/<nome>.rs), instalados em /bin
# ------------------------------------------------------------------------------
$(BUILD_DIR)/user/utilities/%: $(UTILITIES_DIR)/%.rs $(ROOT)/userland/linker.ld \
                                $(USER_API_RLIB) $(USER_RUNTIME_RLIB)
	@mkdir -p $(dir $@)
	$(RUSTC) --crate-name $* --edition 2024 --target $(TARGET) \
		--extern imagineos=$(USER_API_RLIB) --extern imagineos_rt=$(USER_RUNTIME_RLIB) \
		-L dependency=$(USER_API_DEPS) -L dependency=$(USER_RUNTIME_DEPS) \
		-C panic=abort -C relocation-model=static \
		-C link-arg=-T$(ROOT)/userland/linker.ld $< -o $@

# ------------------------------------------------------------------------------
# Runtime C e libc estática
# ------------------------------------------------------------------------------
# Objetos da libc compilados a partir de userland/libraries/libc/*.c
$(BUILD_DIR)/user/%.o: $(USER_LIBC_DIR)/%.c $(USER_LIBC_DIR)/include/dreamcore.h \
                       $(wildcard $(USER_LIBC_INC)/*.h $(USER_LIBC_INC)/sys/*.h)
	@mkdir -p $(dir $@)
	$(CC) $(USER_CFLAGS) -c $< -o $@

$(BUILD_DIR)/user/crt.o: $(USER_CRT_RLIB)
	@mkdir -p $(dir $@)
	cp $(USER_CRT_RLIB) $@

$(BUILD_DIR)/user/crt_empty.o: $(USER_LIBC_DIR)/crt_empty.S
	@mkdir -p $(dir $@)
	$(CC) $(USER_CFLAGS) -c $< -o $@

$(BUILD_DIR)/user/libdreamcore.a: $(USER_LIBC_OBJECTS)
	$(AR) rcs $@ $(USER_LIBC_OBJECTS)

# ------------------------------------------------------------------------------
# Utilitário C: hello_c (compilado com a libc do ImagineOS)
# ------------------------------------------------------------------------------
$(BUILD_DIR)/user/utilities/hello_c: $(UTILITIES_DIR)/hello_c.c $(USER_CRT_OBJECTS) \
                                     $(ROOT)/userland/linker.ld
	@mkdir -p $(BUILD_DIR)/user/c
	$(CC) $(USER_CFLAGS) -c $< -o $(BUILD_DIR)/user/c/hello_c.o
	$(CC) $(USER_CFLAGS) -nostdlib -static -no-pie -Wl,-T$(ROOT)/userland/linker.ld \
		-Wl,--build-id=none $(BUILD_DIR)/user/c/hello_c.o $(USER_CRT_OBJECTS) -o $@

# ------------------------------------------------------------------------------
# Compilação avulsa de um app C de ramfs/home/<nome>.c
# Uso: make c-app APP=<nome>
# ------------------------------------------------------------------------------
c-app:
	@test -n "$(APP)" || (echo "uso: make c-app APP=<nome>"; exit 1)
	$(MAKE) -f $(lastword $(MAKEFILE_LIST)) $(BUILD_DIR)/user/c/$(APP).elf

$(BUILD_DIR)/user/c/%.elf: $(ROOT)/ramfs/home/%.c $(USER_LIBC_DIR)/include/dreamcore.h \
                           $(USER_CRT_OBJECTS) $(ROOT)/userland/linker.ld
	@mkdir -p $(BUILD_DIR)/user/c
	$(CC) $(USER_CFLAGS) -c $< -o $(BUILD_DIR)/user/c/$*.o
	$(CC) $(USER_CFLAGS) -nostdlib -static -no-pie -Wl,-T$(ROOT)/userland/linker.ld \
		-Wl,--build-id=none $(BUILD_DIR)/user/c/$*.o $(USER_CRT_OBJECTS) -o $@
