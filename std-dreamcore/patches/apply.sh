#!/bin/sh
# Aplica a camada `dreamcore` na `std` do toolchain nightly ativo.
#
# A std do Rust escolhe a camada de SO por `target_os`. Em vez de manter um
# fork completo, aplicamos um patch pequeno (117 linhas) em cima da std
# distribuída pelo rustup e adicionamos `os/unix/dreamcore/`.
#
# Uso:
#   std-dreamcore/patches/apply.sh           # aplica
#   std-dreamcore/patches/apply.sh --revert  # restaura o original
#
# Requer `rust-src` instalado (`rustup component add rust-src --toolchain nightly`).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
SRC=$(rustc +nightly --print sysroot)/lib/rustlib/src/rust/library

if [ "${1:-}" = "--revert" ]; then
    if [ -d "$SRC/std.orig.dreamcore" ]; then
        rm -rf "$SRC/std"
        mv "$SRC/std.orig.dreamcore" "$SRC/std"
        echo "std restaurada"
    fi
    if [ -f "$SRC/Cargo.toml.orig.dreamcore" ]; then
        mv "$SRC/Cargo.toml.orig.dreamcore" "$SRC/Cargo.toml"
        mv "$SRC/Cargo.lock.orig.dreamcore" "$SRC/Cargo.lock"
        echo "workspace restaurado"
    fi
    exit 0
fi

[ -d "$SRC/std.orig.dreamcore" ] || cp -a "$SRC/std" "$SRC/std.orig.dreamcore"
[ -f "$SRC/Cargo.toml.orig.dreamcore" ] || cp "$SRC/Cargo.toml" "$SRC/Cargo.toml.orig.dreamcore"
[ -f "$SRC/Cargo.lock.orig.dreamcore" ] || cp "$SRC/Cargo.lock" "$SRC/Cargo.lock.orig.dreamcore"

# Os headers do patch são `std/arquivo`, então aplicamos em `$SRC` com `-p0`:
# o primeiro componente (`std`) já é o diretório real.
(cd "$SRC" && patch -p0 --forward < "$HERE/std-dreamcore.patch" || true)
mkdir -p "$SRC/std/src/os/unix/dreamcore"
cp "$HERE/os-unix-dreamcore/"*.rs "$SRC/std/src/os/unix/dreamcore/"

# A std (build-std) resolve o crate `libc` do registry; aponta para o shim.
if ! grep -q 'std-dreamcore/libc-shim' "$SRC/Cargo.toml"; then
    python3 - "$SRC/Cargo.toml" "$HERE/../libc-shim" <<'PY'
import sys
path, shim = sys.argv[1], sys.argv[2]
text = open(path).read()
text = text.replace("[patch.crates-io]\n",
    "[patch.crates-io]\nlibc = { path = '%s' }\n" % shim, 1)
open(path, "w").write(text)
PY
fi
(cd "$SRC" && cargo +nightly generate-lockfile --offline >/dev/null 2>&1 || true)

# O `build-std` mantém o `libc` compilado em `target/<alvo>/release/build/libc`,
# dentro do diretório de build de cada projeto. Se o Cargo.lock ou o shim mudou,
# esse artefato fica **obsoleto** e a std compila contra a versão antiga do
# shim — sintoma típico: `cannot find function \`xyz\` in crate \`libc\``, mesmo
# com a função presente no fonte.
#
# Não é possível limpar o cache de todos os projetos daqui, então avisamos e
# deixamos o comando pronto. `build.sh` e o `make rust-std` já fazem a limpeza.
built=$(find "$SRC/../../../../.." -maxdepth 6 -type d -path "*/build/libc-*" 2>/dev/null | head -5 || true)
if [ -n "$built" ]; then
    echo
    echo "atenção: existem artefatos de \`libc\` em cache nos projetos abaixo."
    echo "Se a compilação falhar com simbolos ausentes no crate libc, apague o target."
    echo "$built" | sed 's/^/  /'
fi

echo "camada dreamcore aplicada em: $SRC/std"
