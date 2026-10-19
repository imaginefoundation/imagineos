#!/bin/sh
# Cria um projeto Rust configurado para o alvo x86_64-unknown-dreamcore.
#
# Uso:
#   std-dreamcore/new-project.sh <diretório> [nome-do-crate]
#
# O diretório precisa estar vazio (ou não existir). O script gera:
#   Cargo.toml        — perfil release com panic=abort e workspace próprio
#   .cargo/config.toml — alvo, linker, build-std e patch do libc
#   src/main.rs       — programa mínimo com std
#   build.sh          — compila e mostra o caminho do DEF
#
# Depois:
#   cd <diretório> && ./build.sh
set -eu

if [ $# -lt 1 ]; then
    echo "uso: $0 <diretório> [nome-do-crate]" >&2
    exit 2
fi

TARGET_DIR=$1
NAME=${2:-$(basename "$TARGET_DIR")}

# Raiz do ImagineOS: o script vive em `std-dreamcore/`, então a raiz é o pai.
ROOT=$(cd "$(dirname "$0")/.." && pwd)
if [ ! -f "$ROOT/targets/x86_64-unknown-dreamcore.json" ]; then
    echo "erro: não encontrei targets/x86_64-unknown-dreamcore.json em $ROOT" >&2
    exit 1
fi

mkdir -p "$TARGET_DIR/src" "$TARGET_DIR/.cargo"
TARGET_DIR=$(cd "$TARGET_DIR" && pwd)

cat > "$TARGET_DIR/Cargo.toml" <<EOF
[package]
name = "$NAME"
version = "0.1.0"
edition = "2021"

# Projeto independente do workspace do ImagineOS (alvo e build-std próprios).
[workspace]

[profile.release]
panic = "abort"
opt-level = "s"
lto = true
codegen-units = 1
EOF

cat > "$TARGET_DIR/.cargo/config.toml" <<EOF
[build]
target = "$ROOT/targets/x86_64-unknown-dreamcore.json"

[target.x86_64-unknown-dreamcore]
linker = "$ROOT/.build/dnulink"

[unstable]
build-std = ["std"]
build-std-features = ["compiler-builtins-mem"]

# A std do build-std depende de \`libc\`; no ImagineOS ela é o shim sobre a musl.
[patch.crates-io]
libc = { path = "$ROOT/std-dreamcore/libc-shim" }
EOF

cat > "$TARGET_DIR/src/main.rs" <<'EOF'
fn main() {
    println!("olá do ImagineOS!");
}
EOF

cat > "$TARGET_DIR/build.sh" <<EOF
#!/bin/sh
# Compila para DEF. Requer: std-dreamcore/patches/apply.sh já executado e
# \`make -f build/tools.mk dnulink\` feito na raiz do ImagineOS.
set -eu
cd "\$(dirname "\$0")"

# O \`build-std\` reaproveita o \`libc\` compilado em \`target/<alvo>/build/libc\`.
# Se o shim (std-dreamcore/libc-shim) mudou depois da última compilação, esse
# artefato está obsoleto e a std compila contra a versão antiga — sintoma:
# \`cannot find function ... in crate libc\`. Marcamos com um hash.
SHIM="$ROOT/std-dreamcore/libc-shim/src/lib.rs"
STAMP="target/.libc-shim-hash"
HASH=\$( (sha256sum "\$SHIM" 2>/dev/null || echo none) | cut -c1-16)
if [ -f "\$STAMP" ] && [ "\$(cat "\$STAMP")" != "\$HASH" ]; then
    echo "shim libc mudou; limpando cache do build-std..."
    rm -rf target
fi
mkdir -p target && echo "\$HASH" > "\$STAMP"

RUSTC_BOOTSTRAP=1 cargo +nightly build --release -Zjson-target-spec
OUT="target/x86_64-unknown-dreamcore/release/$NAME"
head -c4 "\$OUT" | grep -q DNUF && echo "DEF: \$OUT"
EOF
chmod +x "$TARGET_DIR/build.sh"

echo "projeto criado em: $TARGET_DIR"
echo
echo "pré-requisitos (uma vez, na raiz do ImagineOS):"
echo "  std-dreamcore/patches/apply.sh"
echo "  make musl && make -f build/tools.mk dnulink"
echo
echo "compilar:"
echo "  cd $TARGET_DIR && ./build.sh"
