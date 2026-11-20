# Imagine distro naming specification

**Versão:** 1.0  
**Status:** Rascunho

## Convenções de nomenclatura
- **Imagine/DNU:**
Construída a partir do Kernel DNU, mas usando ferramentas de build do Projeto Imagine (Imagine-Like)
- **ImagineOS/DNU**
Construída a partir de uma build oficial como base para construção de outra distro, também pode se referir à uma build oficial do Projeto Imagine, mas para ser uma distro `SIGNED`, a build recebe uma assinatura específica para builds oficiais.
> **Nota:** Para saber se sua distro é uma distro assinada digite `$> .run(dname) .flags(-s, -sn)`