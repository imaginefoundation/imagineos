# ==============================================================================
# ImagineOS - fontes e utilitários third_party (TinyCC upstream)
# ==============================================================================
#
# Tudo o que depende de código de terceiros fora do controle de versão
# (`third_party/`, ver .gitignore) vive aqui:
#   * checkout/patch da fonte do TinyCC na revisão fixada (TCC_SOURCE_REV);
#   * build do TCC no host (necessário para `make c-app` e para o TCC nativo);
#   * objetos nativos do TCC, compilados para rodar dentro do ImagineOS.
#
# O interruptor THIRD_PARTY (definido em common.mk, padrão 1) desliga o build
# destes utilitários:
#
#   make iso                  -> inclui os utilitários third_party (padrão)
#   make iso THIRD_PARTY=0    -> ISO sem os utilitários third_party
#
# O Limine (`third_party/limine/`) continua obrigatório mesmo com
# THIRD_PARTY=0: sem ele não há bootloader e nenhuma ISO pode ser gerada.
#
# Uso isolado:  make -f build/third_party.mk third-party
# Alvos:        third-party, tcc-source

include $(dir $(lastword $(MAKEFILE_LIST)))common.mk

ifeq ($(THIRD_PARTY),1)

# ------------------------------------------------------------------------------
# TinyCC: fonte, patches e build no host
# ------------------------------------------------------------------------------

.PHONY: third-party tcc-source

$(TCC_SOURCE)/configure:
	@echo "TinyCC source is missing. Run 'make tcc-source' to fetch the pinned upstream revision."
	@false

tcc-source:
	@if [ ! -x "$(TCC_SOURCE)/configure" ]; then \
		git clone https://github.com/TinyCC/tinycc.git "$(TCC_SOURCE)" && \
		git -C "$(TCC_SOURCE)" checkout --detach "$(TCC_SOURCE_REV)"; \
	fi

$(TCC_CONFIG_STAMP): $(TCC_SOURCE)/configure $(ROOT)/Makefile
	@mkdir -p $(BUILD_DIR)/toolchain
	cd $(TCC_SOURCE) && ./configure --prefix=/usr --tccdir=/usr/lib/tcc \
		--sysincludepaths=/usr/include:/usr/include/x86_64-linux-gnu:$(shell $(CC) -print-file-name=include) \
		--libpaths=/usr/lib:/usr/lib/x86_64-linux-gnu \
		--crtprefix=/usr/lib/x86_64-linux-gnu:/usr/lib \
		--config-bcheck=no --config-backtrace=no
	touch $@

$(TCC_PATCH_STAMP): $(ROOT)/userland/patches/tinycc-imagineos.patch $(TCC_SOURCE)/tccrun.c $(TCC_CONFIG_STAMP)
	@if ! grep -Fq 'TCCSYM(gettimeofday)' $(TCC_SOURCE)/tccrun.c; then \
		patch --forward -p1 -d $(TCC_SOURCE) < $<; \
	fi
	touch $@

$(TCC_HOST): $(TCC_CONFIG_STAMP) $(TCC_PATCH_STAMP) $(TCC_SOURCE)/Makefile $(TCC_SOURCE)/tcc.c
	@mkdir -p $(BUILD_DIR)/toolchain
	$(MAKE) -C $(TCC_SOURCE)
	cp $(TCC_SOURCE)/tcc $@

$(TCC_SOURCE)/libtcc1.a: $(TCC_HOST)
	@test -f $@

$(TCC_SOURCE)/runmain.o: $(TCC_HOST)
	@test -f $@

# ------------------------------------------------------------------------------
# TinyCC nativo (roda dentro do ImagineOS)
# ------------------------------------------------------------------------------

$(BUILD_DIR)/user/tcc/%.o: $(TCC_SOURCE)/%.c $(TCC_HOST) $(TCC_PATCH_STAMP) \
                            $(wildcard $(ROOT)/userland/libc/include/*.h $(ROOT)/userland/libc/include/sys/*.h)
	@mkdir -p $(dir $@)
	$(CC) $(TCC_NATIVE_CFLAGS) -c $< -o $@

$(BUILD_DIR)/user/utilities/tcc: $(TCC_NATIVE_OBJECTS) $(USER_CRT_OBJECTS) $(ROOT)/userland/linker.ld
	@mkdir -p $(dir $@)
	$(CC) $(USER_CFLAGS) -nostdlib -static -no-pie \
		-Wl,-T,$(ROOT)/userland/linker.ld -Wl,--build-id=none \
		$(TCC_NATIVE_OBJECTS) $(USER_CRT_OBJECTS) -o $@

# Alvo agregador: tudo que third_party produz.
third-party: $(TCC_HOST) $(TCC_SOURCE)/libtcc1.a $(TCC_SOURCE)/runmain.o \
             $(BUILD_DIR)/user/utilities/tcc

else

# THIRD_PARTY=0: alvos presentes, mas sem efeito de build.
.PHONY: third-party tcc-source

third-party tcc-source:
	@echo "THIRD_PARTY=0: utilitários third_party desativados (nada a compilar)."

endif
