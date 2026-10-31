# ImagineOS Runtime — DEF Format & Application Packaging

Este documento descreve o **runtime de execução** do ImagineOS: o formato binário
**DEF** (Dreamcore Executable Format), o caminho de carregamento no kernel, e a
ferramenta de empacotamento `dnu-pack`.

> **Nota:** o DEF **não** usa extensão de arquivo. Programas de sistema e
> utilitários são arquivos sem sufixo (`/sbin/init`, `/bin/ls`). A extensão
> `.xapp` é do **pacote de aplicação** do framework Liminal
> (`documentation/appframework.md`), que é outro artefato: um arquivo de
distribuição interpretado pela Liminal Runtime, e **não** um executável DEF.

---

## 1. Visão Geral

O ImagineOS executa aplicativos em Ring 3 carregados a partir de um formato
binário próprio chamado **DEF**. É o único formato aceito pelo kernel: o loader
ELF de userspace foi removido da ABI.

O fluxo completo é:

```text
[ fontes do app ] --dnu-pack--> [ DEF sem extensão ] --exec--> [ processo em Ring 3 ]
```

---

## 2. O Formato DEF

O DEF é um contêiner executável simples e verificável, definido em
`common/shared/abi/src/def.rs`. Ele é composto por:

1. **Cabeçalho (`DefHeader`)** — identifica o formato, a arquitetura, o tipo de
   artefato e os metadados de layout.
2. **Tabela de segmentos (`DefSegment`)** — uma entrada por segmento carregável.
3. **Payload** — os bytes dos segmentos, armazenados logo após a tabela.

### 2.1 Layout de bytes

| Offset | Tamanho | Campo | Descrição |
|-------:|--------:|-------|-----------|
| `0`  | 4  | `magic`            | `DEF_MAGIC` (`"DNUF"`) |
| `4`  | 1  | `arch`             | `DefArch` (ex.: `X86_64`) |
| `5`  | 1  | `type`             | `DefType` (ex.: `App`) |
| `6`  | 2  | `format_version`   | `DEF_FORMAT_VERSION` |
| `8`  | 4  | `abi_version`      | `ABI_VERSION` esperada |
| `12` | 4  | `flags`            | Reservado |
| `16` | 4  | `segment_count`    | Número de entradas na tabela |
| `20` | 4  | `reserved`         | Reservado |
| `24` | 8  | `entry_point`      | Endereço virtual de entrada |
| `32` | 8  | `stack_size`       | Tamanho da pilha do processo |
| `40` | 8  | `image_base`       | Base de carga da imagem |
| `48` | 8  | `image_size`       | Tamanho total da imagem |
| `56` | —  | *tabela de segmentos* | `DEF_SEGMENT_SIZE` bytes cada |

### 2.2 Entrada de segmento (`DefSegment`)

| Offset | Tamanho | Campo | Descrição |
|-------:|--------:|-------|-----------|
| `0`  | 8  | `file_offset` | Offset **absoluto** no arquivo DEF |
| `8`  | 8  | `file_size`   | Bytes a ler do arquivo |
| `16` | 8  | `memory_size` | Bytes a mapear em memória (≥ `file_size`) |
| `24` | 8  | `vaddr`       | Endereço virtual de destino |
| `32` | 4  | `permissions` | Flags `DefPermissions` (READ/WRITE/EXECUTE) |
| `36` | 4  | `reserved`    | Reservado |

> **Importante:** `file_offset` é **absoluto** em relação ao início do arquivo
> DEF. O payload começa em `DEF_HEADER_SIZE + DEF_SEGMENT_SIZE`, ou seja, logo
> após a tabela de segmentos. Um leitor **não deve** somar novamente o início da
> tabela ao `file_offset`.

---

## 3. Carregamento no Kernel

O carregador de DEF vive em `dnu/src/exec_def.rs` (repositório DNU) e é o ponto de entrada de
todo executável em Ring 3. O processo de carga é:

1. **Leitura e validação do cabeçalho** — confere `magic`, `arch`, `type`,
   `format_version` e `abi_version`. Qualquer divergência gera
   `Error::InvalidMagic`, `Error::UnsupportedArch`, `Error::BadFormatVersion`
   ou `Error::AbiMismatch`.
2. **Iteração sobre a tabela de segmentos** — para cada entrada, calcula a
   região de destino em memória e copia os bytes do payload usando
   `file_offset` **absoluto**.
3. **Mapeamento de páginas** — aloca e mapeia cada segmento com as permissões
   declaradas (`DefPermissions`), garantindo que segmentos somente-leitura e
   somente-execução não sejam graváveis.
4. **Alocação da pilha** — reserva `stack_size` bytes em `stack_top` e prepara
   o ponteiro de pilha inicial.
5. **Salto para `entry_point`** — transfere o controle para o endereço de
   entrada em Ring 3.

### 3.1 Tratamento de erros

O parser é defensivo contra arquivos malformados ou maliciosos. Erros possíveis
incluem: `Truncated` (arquivo menor que o cabeçalho ou que a tabela declara),
`BadSegmentCount` (contagem de segmentos absurda), `Overflow` (somas de
`vaddr`/`memory_size` estourando), `Permissions` (flags inválidas) e
`OutOfMemory` durante a alocação de páginas.

### 3.2 Verificação de integridade

O `app.json` gerado no build carrega o hash de integridade do pacote. O runtime
deve comparar esse hash com o conteúdo do DEF antes de executá-lo, recusando
pacotes adulterados ou corrompidos.

---

## 4. Empacotamento: `dnu-pack`

O `dnu-pack` (fonte em `dnu/tools/dnu-pack.rs`, compilado por `dnu/Makefile` e promovido
por `build/tools.mk`) é a
ferramenta de linha de comando que transforma fontes em um arquivo DEF.

### 4.1 Uso

```bash
dnu-pack <entrada.elf> <saida> [--type app|server|driver|runtime] [--caps ...] [--stack <bytes>]
```

O `dnu-pack` recebe o **ELF estático** que o `rustc`/`cc` produzem (é o único
formato que o toolchain sabe emitir) e reescreve os segmentos `PT_LOAD` no
contêiner DEF. Depois disso, o ELF original não é mais usado: o kernel lê apenas
DEF.

### 4.2 O que a ferramenta faz

1. Lê o binário/objeto de entrada e resolve seu ponto de entrada.
2. Divide o conteúdo em segmentos, atribuindo a cada um suas permissões
   (`READ`, `WRITE`, `EXECUTE`).
3. Monta o cabeçalho DEF, a tabela de segmentos e o payload, ajustando cada
   `file_offset` para o valor **absoluto** correto.
4. Escreve o arquivo final (sem extensão), pronto para ser carregado pelo kernel.

---

## 5. Próximos Passos

* [ ] Implementar a verificação de hash do `app.json` no runtime.
* [ ] Suportar relocação de segmentos dependentes de `image_base`.
* [ ] Expor o tipo de artefato `Driver` para drivers em espaço de usuário
      (ver `documentation/drivers.md`).
* [ ] Integrar o `dnu-pack` ao `framework-cli build` do Liminal.

---

*Documentação mantida pelo Imagine OS Team.*
