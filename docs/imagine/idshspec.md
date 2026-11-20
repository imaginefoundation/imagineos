# Imagine Project "DSH" — DotShell Specification

**Versão:** 1.0  
**Status:** Rascunho  
**Copyright (C) 2026 - The Imagine Project**  
**Aplica-se a:** Interpretador de comandos DSH (DotShell) do userspace do ImagineOS

---

## 1. Introdução

O **DSH (DotShell)** é o interpretador de comandos padrão do userspace do ImagineOS. Diferente de shells tradicionais (sh, bash), o DSH adota uma sintaxe baseada em **pontos** (dot-notation), onde cada comando ou operação é prefixado por `.` e argumentos são passos dentro de parênteses. O objetivo é uma sintaxe simples, de fácil parsing e extensível para scripts de sistema, inicialização e automação de tarefas no userspace.

O DSH roda inteiramente em **userspace (Ring 3)** e se comunica com o kernel DNU via syscalls definidas em `dnu/abi/src/syscall.rs`. Não há código de shell no kernel, conforme a regra de ouro do projeto: GUI e shell pertencem ao userspace.

---

## 2. Sintaxe

### 2.1 Forma geral

Um comando DSH segue o padrão:

```
$> .comando("argumento1", "argumento2")
```

| Elemento | Descrição |
|----------|-----------|
| `.` | Prefixo obrigatório de todo comando/função DSH |
| `comando` | Nome da palavra reservada ou built-in |
| `()` | Escopo da função / delimitador de argumentos |
| `"argumento"` | Valor literal (string) passado ao comando |

### 2.2 Exemplos sintáticos

```dsh
$> .run("programa")                     # Executa um programa
$> .from("cow").run("moo")              # Comando específico de um programa
$> .run("cow").args(owner="root")       # Argumentos nomeados
$> .run("cow").flags(-sound="moo")      # Flags do programa
```

Quando há múltiplos programas ou argumentos, eles são separados por vírgula dentro dos parênteses:

```
$> .run("program1", "program2")
$> .args(a="1", b="2")
```

### 2.3 Literais

| Tipo | Formato | Exemplo |
|------|---------|---------|
| String | `"..."` | `"programa"`, `"root"` |
| Inteiro | dígitos | `42`, `-7` |
| Booleano | `true` / `false` | `true` |
| Identificador | letra ou `_`, seguido de alfanuméricos | `minha_var`, `_privado` |

Strings suportam sequências de escape: `\n`, `\t`, `\\`, `\"`.

### 2.4 Comentários

Comentários de linha começam com `#` e vão até o final da linha:

```dsh
# Isto é um comentário de linha
.run("programa")   # comentário inline
```

---

## 3. Palavras Reservadas (Built-ins)

Todas as palavras reservadas começam com `.` e aceitam argumentos dentro de parênteses.

| Comando | Assinatura | Descrição |
|---------|-----------|-----------|
| `.run()` | `.run("programa")` | Executa um programa shell/userspace |
| `.from()` | `.from("comando")` | Seleciona um comando específico dentro de um programa |
| `.args()` | `.args(chave="valor", ...)` | Define argumentos nomeados para o programa |
| `.flags()` | `.flags(-chave="valor", ...)` | Define flags (opções) para o programa |
| `.loop()` | `.loop(n)` | Repete um bloco `n` vezes |
| `.if()` | `.if(condicao)` | Inicia um bloco condicional |
| `.endif()` | `.endif()` | Fecha um bloco `.if()` |
| `.else()` | `.else()` | Bloco alternativo de `.if()` |
| `.scope()` | `.scope()` | Abre um novo escopo de variáveis |
| `.import()` | `.import("modulo")` | Importa um módulo/biblioteca de scripts |
| `.var()` | `.var(nome = valor)` | Declara uma variável no escopo atual |
| `.setvar()` | `.setvar(nome = valor)` | Atualiza o valor de uma variável existente |
| `.function()` | `.function(nome) { ... }` | Define uma função reutilizável |
| `.pipe()` | `.pipe("programa")` | Passa a saída do programa anterior como entrada/argumento do próximo |

---

## 4. Detalhes dos Comandos

### 4.1 `.run("programa")`

Executa o programa nomeado. Se não houver programa anterior selecionado via `.from()`, `.run()` busca o programa no PATH do sistema.

```dsh
$> .run("ls")
$> .run("cat").args(arquivo="/etc/distro-release")
```

### 4.2 `.from("comando")`

Seleciona um comando específico (builtin ou subcomando) de um programa. Útil para programas que oferecem múltiplos modos de operação.

```dsh
$> .from("cow").run("moo")
# Executa o comando "moo" do programa "cow"
```

### 4.3 `.args(chave="valor", ...)`

Define argumentos nomeados (parâmetros) para o programa que será executado.

```dsh
$> .run("cow").args(owner="root", colors="black, white", style="dotted")
```

### 4.4 `.flags(-chave="valor", ...)`

Define flags (opções de linha de comando) para o programa. Flags usam prefixo `-`.

```dsh
$> .run("cow").flags(-sound="moo", -food="wheat", -drinks="water")
```

### 4.5 `.loop(n)`

Repete o bloco seguinte `n` vezes. O bloco é encerrado implicitamente no fim da repetição.

```dsh
$> .loop(3)
    .run("echo").args(msg="ola")
```

### 4.6 `.if(condicao)` / `.else()` / `.endif()`

Bloco condicional. A condição é uma expressão booleana.

```dsh
$> .if(uid == 0)
    .run("whoami")
.else()
    .run("echo").args(msg="usuario nao root")
.endif()
```

### 4.7 `.scope()`

Abre um novo escopo de variáveis. Variáveis declaradas dentro de um `.scope()` não vazam para o escopo externo.

```dsh
$> .var(x = 10)
.scope()
    .var(x = 20)
    .run("echo").args(msg=x)   # imprime 20
.endscope()
.run("echo").args(msg=x)       # imprime 10
```

### 4.8 `.import("modulo")`

Importa um módulo de script DSH, tornando suas funções e variáveis disponíveis no escopo atual.

```dsh
$> .import("utilidades")
.utilidades.funcao_auxiliar()
```

### 4.9 `.var(nome = valor)` / `.setvar(nome = valor)`

Declara (`.var`) ou atualiza (`.setvar`) uma variável.

```dsh
$> .var(nome = "adryan")
$> .setvar(nome = "root")
```

### 4.10 `.function(nome) { ... }`

Define uma função reutilizável. O corpo da função fica entre chaves `{}`.

```dsh
$> .function(saudacao) {
    .run("echo").args(msg="Olá, mundo!")
}
.saudacao()
```

### 4.11 `.pipe("programa")`

Passa a saída padrão (`stdout`) do programa executado anteriormente como entrada ou argumento do próximo programa. É o equivalente do DSH ao pipe (`|`) de shells tradicionais.

Como o DSH não usa separador de pipe (`|`), o encadeamento se dá por chamada sequente: o primeiro `.run()` executa um programa e captura sua saída; o segundo `.run()` (ou `.pipe()`) recebe essa saída como entrada.

```dsh
# Exemplo 1: ls seguido de grep (simulado)
$> .run("ls")
    .args(diretorio="/etc")
$> .run("grep").args(padrao="passwd")

# Exemplo 2: cat seguido de wc (contagem de linhas)
$> .run("cat").args(arquivo="/var/log/sistema.log")
$> .run("wc").args(opcao="-l")

# Exemplo 3: saida de um programa como argumento posicional de outro
$> .run("gerar_id")
$> .run("usar_id").args($?)
```

**Como um programa recebe o resultado de outro:**

O DSH expõe o `stdout` do último programa executado na variável implícita `$stdout` (ou `$out`). Um programa pode recebê-la de três formas:

1. **Como argumento nomeado:** `.args(entrada=$stdout)`
2. **Como argumento posicional:** `.args($stdout)` ou direto `.run("programa").args($stdout)`
3. **Via variável explícita:** `.var(entrada = $stdout)` seguido de `.run("programa").args(entrada)`

Exemplo completo:

```dsh
$> .run("data_atual")
$> .var(hora = $stdout)
$> .run("registrar").args(hora=$hora)
```

---

## 5. Exemplos

### 5.1 Execução simples

```dsh
$> .run("ls")
$> .run("cat").args(arquivo="/etc/distro-release")
```

### 5.2 Programa com argumentos e flags

```dsh
$> .run("cow")
    .args(owner="root", colors="black, white")
    .flags(-sound="moo", -food="wheat")
```

### 5.3 Comando específico de um programa

```dsh
$> .from("cow").run("moo")
```

### 5.4 Script condicional com loop

```dsh
$> .var(tentativas = 0)
.loop(5)
    .if(tentativas < 3)
        .run("echo").args(msg="tentando...")
        .setvar(tentativas = tentativas + 1)
    .else()
        .run("echo").args(msg="falhou apos 3 tentativas")
    .endif()
```

### 5.5 Uso de funcoes e escopo

```dsh
$> .function(instalar_pacote) {
    .run("apt").args(pacote=$1).flags(-y)
}
.scope()
    .var(pacote = "vim")
    .instalar_pacote(pacote)
.endscope()
```

### 5.6 Script completo de inicializacao (`init.dsh`)

```dsh
# Script de inicializacao do userspace ImagineOS

# Configura idioma e teclado
.var(idioma = "pt_BR")
.var(layout = "abnt2")

# Monta sistemas de arquivos
.run("mount").args(fonte="/dev/hda", destino="/mnt")
.run("mkdir").args(caminho="/mnt/home")

# Inicia servicos essenciais
.function(iniciar_servico) {
    .run($1).flags(-daemon)
}
.iniciar_servico("ws_server")
.iniciar_servico("acpid")

# Mensagem de boas-vindas
.run("echo").args(msg="Bem-vindo ao ImagineOS!")
```

---

## 6. Semantica de Execucao

1. **Ordem de avaliacao:** os comandos sao executados da esquerda para a direita, na ordem em que aparecem no script.
2. **Estado acumulado:** cada `.comando(...)` pode modificar o estado do interpretador (variaveis, programa selecionado, argumentos pendentes).
3. **Execucao disparada por `.run()`:** o programa selecionado (via `.from()` ou implicito) e efetivamente executado apenas quando `.run()` e chamado. Argumentos e flags definidos antes de `.run()` sao aplicados a essa execucao.
4. **Escopo de variaveis:** variaveis sao resolvidas no escopo mais proximo. `.scope()` cria um novo nível; variaveis internas sombreiam variaveis externas com o mesmo nome.
5. **Codigo de saida:** todo programa executado retorna um codigo de saida (0 = sucesso, nao-zero = erro). O DSH expõe o codigo da ultima execucao na variavel implicita `$?`.
6. **Saída padrão:** o `stdout` do último programa executado fica disponível na variável implícita `$stdout`, permitindo piping entre programas via `.args($stdout)` ou `.var(x = $stdout)`.

---

## 7. Tratamento de Erros

| Erro | Comportamento |
|------|---------------|
| Comando nao encontrado | O DSH imprime `comando nao encontrado: <nome>` e retorna codigo 127 |
| Erro de sintaxe | O parser interrompe o script e imprime a linha e coluna do erro |
| Erro de execucao | Se o programa alvo retorna nao-zero, o DSH continua o script por padrão. Use `.if($? != 0)` para tratar |
| Variavel nao definida | Acessar variavel nao definida resulta em string vazia e aviso no log |

---

## 8. Comparacao com Shells Tradicionais

| Caracteristica | DSH | sh/bash |
|----------------|-----|---------|
| Prefixo de comandos | `.` (obrigatorio) | nenhum |
| Delimitador de argumentos | `()` | espacos |
| Argumentos nomeados | `.args(chave="valor")` | `$1`, `$2`, ... |
| Flags | `.flags(-chave="valor")` | `-chave valor` |
| Encadeamento / Piping | `.run()` seguido de `.run()` (saída via `$stdout`) | `\|` (pipe) |
| Escopo de variaveis | `.scope()` | subshells `()` |
| Execucao disparada | `.run()` | linha de comando direta |
| Saída do programa anterior | Variável implícita `$stdout` | `$()` (substituição de comando) |

---

## 9. Referencias

- [`dnu/abi/src/syscall.rs`](../../../dnu/abi/src/syscall.rs) — Numeros de syscall e codigos de erro
- [`docs/dnu/defspec.md`](../dnu/defspec.md) — Formato de executavel DEF
- [`docs/dnu/process.md`](../dnu/process.md) — Gerenciamento de processos
- [`docs/dnu/fsspec.md`](../dnu/fsspec.md) — Sistemas de arquivos (VFS, RAMFS, DFS)
- [`AGENTS.md`](../../../AGENTS.md) — Guia de desenvolvimento do ImagineOS

---