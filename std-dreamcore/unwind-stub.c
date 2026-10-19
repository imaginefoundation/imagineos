/*
 * Stubs de unwinding para o alvo x86_64-unknown-dreamcore.
 *
 * A `std` compila `panic = "abort"`, mas o compilador ainda gera a função
 * `rust_eh_personality` (referenciada pelas tabelas `.eh_frame` que o script
 * descarta) e alguns caminhos do `core` chamam `_Unwind_*`. Sem estes símbolos
 * o link falha; com eles, qualquer tentativa real de desenrolar a pilha
 * termina o processo — que é exatamente o contrato de `panic = "abort"`.
 *
 * Não há `libunwind` no ImagineOS: mantê-los como stubs é mais honesto do que
 * linkar um runtime de unwinding que nunca vai conseguir andar pelas pilhas
 * do kernel.
 */
#include <stdint.h>

void abort(void);

struct _Unwind_Context;
typedef int _Unwind_Reason_Code;

_Unwind_Reason_Code rust_eh_personality(int version, int actions, uint64_t exn_class,
                                        void *exn, struct _Unwind_Context *ctx) {
    (void)version; (void)actions; (void)exn_class; (void)exn; (void)ctx;
    return 0; /* _URC_CONTINUE_UNWIND */
}

void *_Unwind_GetLanguageSpecificData(struct _Unwind_Context *ctx) { (void)ctx; return 0; }
uintptr_t _Unwind_GetRegionStart(struct _Unwind_Context *ctx) { (void)ctx; return 0; }
uintptr_t _Unwind_GetIPInfo(struct _Unwind_Context *ctx, int *before) {
    (void)ctx; if (before) *before = 0; return 0;
}
uintptr_t _Unwind_GetTextRelBase(struct _Unwind_Context *ctx) { (void)ctx; return 0; }
uintptr_t _Unwind_GetDataRelBase(struct _Unwind_Context *ctx) { (void)ctx; return 0; }
void _Unwind_SetGR(struct _Unwind_Context *ctx, int index, uintptr_t value) {
    (void)ctx; (void)index; (void)value;
}
void _Unwind_SetIP(struct _Unwind_Context *ctx, uintptr_t value) { (void)ctx; (void)value; }
void _Unwind_Resume(struct _Unwind_Context *ctx) { (void)ctx; abort(); }
void _Unwind_DeleteException(void *exn) { (void)exn; }

/* Alias de `errno` usado por fragmentos do `core` sem os underscores. */
extern int *__errno_location(void);
int *errno_location(void) { return __errno_location(); }
