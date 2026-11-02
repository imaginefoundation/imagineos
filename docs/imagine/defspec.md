# Formato DEF (Dreamcore _"DNU"_ Executable Format)

O **DEF** é um formato binário composto por um cabeçalho, uma tabela de segmentos e os dados dos segmentos.

## Cabeçalho

O cabeçalho tem 56 bytes e contém os seguintes campos:

| Offset | Tamanho | Campo | Descrição |
|---:|---:|---|---|
| 0 | 4 bytes | `magic` | Identificador do formato: `DEF_MAGIC` (`"DNUF"`) |
| 4 | 1 byte | `arch` | Arquitetura, representada por `DefArch` |
| 5 | 1 byte | `type` | Tipo de artefato, representado por `DefType` |
| 6 | 2 bytes | `format_version` | Versão do formato DEF |
| 8 | 4 bytes | `abi_version` | Versão da ABI |
| 12 | 4 bytes | `flags` | Reservado |
| 16 | 4 bytes | `segment_count` | Número de entradas na tabela de segmentos |
| 20 | 4 bytes | `reserved` | Reservado |
| 24 | 8 bytes | `entry_point` | Endereço virtual do ponto de entrada |
| 32 | 8 bytes | `stack_size` | Tamanho da pilha indicado no cabeçalho |
| 40 | 8 bytes | `image_base` | Endereço-base da imagem |
| 48 | 8 bytes | `image_size` | Tamanho total da imagem |

## Entradas de segmento

Cada entrada da tabela tem 40 bytes. A tabela começa no offset 56, imediatamente após o cabeçalho.

| Offset na entrada | Tamanho | Campo | Descrição |
|---:|---:|---|---|
| 0 | 8 bytes | `file_offset` | Offset absoluto do segmento no arquivo DEF |
| 8 | 8 bytes | `file_size` | Tamanho dos dados do segmento no arquivo |
| 16 | 8 bytes | `memory_size` | Tamanho reservado para o segmento em memória; deve ser maior ou igual a `file_size` |
| 24 | 8 bytes | `vaddr` | Endereço virtual do segmento |
| 32 | 4 bytes | `permissions` | Permissões representadas por `DefPermissions` |
| 36 | 4 bytes | `reserved` | Reservado |

As permissões de segmento incluem `READ`, `WRITE` e `EXECUTE`.

## Organização dos dados

A tabela contém `segment_count` entradas, cada uma com `DEF_SEGMENT_SIZE` bytes. O payload dos segmentos começa imediatamente após o cabeçalho e a tabela.

`file_offset` é relativo ao início do arquivo DEF. Portanto, leitores devem usá-lo como offset absoluto, sem somar novamente o tamanho do cabeçalho ou o início da tabela.

Em termos gerais, o arquivo tem esta organização:

```text
[cabeçalho][tabela de segmentos][dados dos segmentos]
```

## Constantes e tipos

Os valores de `DEF_MAGIC`, `DEF_FORMAT_VERSION`, `ABI_VERSION`, `DEF_HEADER_SIZE`, `DEF_SEGMENT_SIZE`, `DefArch`, `DefType` e `DefPermissions` são definidos em `imagine-common/abi/src/def.rs`.