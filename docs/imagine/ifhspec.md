# Especificação da Hierarquia do Sistema de Arquivos do ImagineOS

**Versão:** 1.0  
**Status:** Rascunho

**Nota:** Atualmente em `docs/imagine`, mas o sistema de arquivos baseado em banco de dados é implementação do DNU, aqui ficará somente a especificação sobre a hierarquia do Sistema de Arquivos

# Rascunho do layout da hierarquia do sistema de arquivos
Para sistemas baseados em `Imagine/DNU*`, `Generic/DNU*` e `ImagineOS/DNU*`.

```
/ (Root Directory)
|_ System/
|	|_ Users/
|		|_ (USER_NAME_1)
|		|		|_ Home/
|		|		|_ local#Applications/
|		|		|_ local#Libraries/
|		|		|_ local#Config/
|		|		|_ local#Variables/
|		|_ (USER_NAME_2)
|		|_ (USER_NAME_3)
|		|_ .ADMIN
|		|_ .ROOT
|		|_ USERS.db
|_ Boot
|		|_ Config
|		|_ Kernel
|		|_ Bin
|		|_ Loader
|_ Tools
|_ Devices
|_ Drivers
|_ Mounted
|_ Config
|		|_ global#Global
|		|_ global#Variables
|		|_ Registry
|_ Libraries
|  |_ PROG1
|	 |	|_ @lib1.dll
|  |_ PROG2
|	 |	|_ @lib1.dll
|  |_ PROG3
|  |  |_ libx.dll
|  |_ Shared
|  |  |_ lib1.dll
|  |_ System
|     |_ syslib1.dll
|     |_ syslib2.dll
|     |_ syslib3.dll
|     |_ syslibx.dll
|_ Applications/
   |_ pub.imagine.helloworld
   |_ pub.imagine.testing
   |_ pub.imagine.hi
```
> **OBSERVAÇÃO:** Para sistemas indicados com `*`, verificar a especificação `idistrospec.md`.
---

# Rascunho: Sistema de Arquivos Baseado em Banco de Dados

> **Status:** Ideia em estágio inicial, não implementada.  
> Esta seção explora como o ImagineOS poderia evoluir para um modelo de sistema de arquivos onde arquivos e diretórios são registros em um banco de atributos.

## Motivação

Um sistema de arquivos tradicional armazena apenas nome, conteúdo e poucos metadados (tamanho, data). Um modelo baseado em banco de dados permite que cada arquivo seja um registro com **atributos arbitrários** (autor, tags, tipo MIME, cor, versão, descrição), consultáveis diretamente.

## Princípios

- O **RAMFS/DFS existente continua sendo a camada de blocos** (armazenamento físico de bytes).
- Uma **camada de banco de atributos** fica sobre essa camada, indexando arquivos por nome, pai e atributos chave-valor.
- Buscas passam a ser consultas (ex: "todos os arquivos com atributo `autor=adryan`") em vez de apenas percorrer pastas.

## Mapeamento para a IFHSpec

A árvore definida acima continua valendo como a **visão hierárquica**, mas cada entrada pode ser interpretada como um registro no banco:

```
/ (raiz)
|_ System/Users/<usuario>/      ← esquema de usuário no banco
|   |_ Home/                    ← arquivos do usuário (registros com atributos)
|   |_ local#Applications/      ← aplicações registradas com metadados
|   |_ local#Libraries/         ← bibliotecas com dependências declaradas
|   |_ local#Config/            ← configurações como pares chave-valor
|   |_ local#Variables/         ← variáveis de ambiente como registros
|_ Config/Registry/             ← banco global de configurações do sistema
|_ Libraries/System/            ← bibliotecas do sistema com metadados de versão
|_ Applications/pub.imagine.*   ← cada app é um registro com ícone, descrição, versão
```

## Tabelas sugeridas (conceitual)

```sql
arquivo (
    id          INTEGER PRIMARY KEY,
    pai_id      INTEGER,          -- NULL se raiz
    nome        TEXT,
    tipo        TEXT,             -- 'arquivo' | 'diretorio' | 'aplicacao' | 'biblioteca'
    tamanho     INTEGER,
    criado_em   INTEGER,
    modificado_em INTEGER,
    bloco_inicial INTEGER,        -- ponteiro para dados no RAMFS/DFS
    dono_uid    INTEGER
)

atributo (
    arquivo_id  INTEGER,
    chave       TEXT,             -- ex: 'autor', 'mime', 'descricao'
    valor       TEXT
)
```

## Syscalls sugeridas (futuro)

| Syscall | Descrição |
|---------|-----------|
| `SYS_ATTR_SET` | Define um atributo (chave=valor) em um arquivo |
| `SYS_ATTR_GET` | Lê um atributo de um arquivo |
| `SYS_ATTR_LIST` | Lista atributos de um arquivo |
| `SYS_ATTR_QUERY` | Busca arquivos por atributo |

## Escopo recomendado para o ImagineOS

- **Não** implementar SQL completo no kernel (o kernel é `no_std` e roda em modo bare-metal).
- Começar com um **arquivo de índice binário** no RAMFS (ex: `/System/Users/<usuario>/.indice.bin`) contendo (nome, bloco, tamanho) e (arquivo_id, chave, valor).
- Manter consultas simples (inserção, leitura, busca por chave/valor); consultas complexas podem ficar para ferramentas de userspace.