#!/usr/bin/env python3
"""build.py — ponto de entrada do build do ImagineOS.

Wrapper que delega para o orquestrador de build em
imagine-tools/build/build.py.

Uso:
    python3 build.py ramfs          # monta a árvore do ramfs
    python3 build.py userland       # compila programas de userspace
    python3 build.py kernel         # compila o dzImage (kernel + initramfs)
    python3 build.py components     # compila componentes dos submódulos
    python3 build.py iso            # compila tudo e monta a ISO inicializável
    python3 build.py run            # compila a ISO e inicia no QEMU/OVMF
    python3 build.py clean          # remove artefatos de build
    python3 build.py tools          # compila ferramentas de host (dnu-pack, etc.)
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BUILD_SCRIPTS = ROOT / "imagine-tools" / "build" / "build.py"


def main() -> int:
    if not BUILD_SCRIPTS.exists():
        print(f"erro: script de build não encontrado em {BUILD_SCRIPTS}", file=sys.stderr)
        return 1

    # Passa todos os argumentos para o script de build real
    cmd = [sys.executable, str(BUILD_SCRIPTS)] + sys.argv[1:]
    result = subprocess.run(cmd, cwd=str(ROOT))
    return result.returncode


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("\ninterrompido")
        raise SystemExit(130)
