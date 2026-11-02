# Imagine Application Framework (IAF)

Este documento especifica a estrutura de um aplicativo de *userland* compatível com o **Imagine Application Framework**. O formato descreve os metadados do aplicativo, seus arquivos e sua janela principal. Não define como o sistema operacional implementa ou apresenta janelas.

## 1. Estrutura do projeto

Um projeto de aplicativo segue esta estrutura:

```text
meu-app/
├── AppManifest.xml
└── src/
    ├── main.aml
    ├── main.css
    └── main.lua
```

- `AppManifest.xml`: identifica o aplicativo, sua versão e seus pontos de entrada.
- `main.aml`: descreve a interface do aplicativo.
- `main.css`: contém estilos da interface.
- `main.lua`: contém a lógica do aplicativo.

Os arquivos de interface e lógica são opcionais quando não forem necessários. O manifesto é obrigatório.

## 2. Manifesto do aplicativo

O manifesto declara os metadados e a janela principal:

```xml
<Application name="Bloco de Notas" version="1.0" entry="src/main.lua">
    <Window width="400" height="300" title="Meu Bloco de Notas"/>
</Application>
```

O atributo `entry` indica o arquivo de entrada da lógica do aplicativo. Os atributos `width`, `height` e `title` descrevem a janela solicitada pelo aplicativo.

## 3. Arquivos do aplicativo

### `src/main.aml`

O arquivo AML descreve o conteúdo da interface:

```xml
<aml>
  <head>
    <title>Bloco de Notas</title>
    <meta name="developer" content="com.example"/>
  </head>
  <body>
    <textarea id="editor" placeholder="Digite algo aqui..."/>
    <button id="btn-salvar" text="Salvar arquivo"/>
  </body>
</aml>
```

### `src/main.css`

O arquivo CSS define a apresentação dos elementos da interface:

```css
body {
    padding: 12px;
}

button {
    margin-top: 8px;
}
```

### `src/main.lua`

O arquivo Lua contém a lógica do aplicativo e pode interagir com os elementos declarados na interface, conforme as APIs disponibilizadas pelo ambiente de execução do sistema:

```lua
local UI = require("imagine.application")

local btn = UI.get_element_by_id("btn-salvar")
local editor = UI.get_element_by_id("editor")

btn:on_click(function()
    print("Texto atual: " .. editor:get_text())
end)
```

Os nomes e as funções dos módulos usados por um aplicativo dependem das APIs disponíveis na versão do sistema e do framework.

## 4. Empacotamento

Um aplicativo pode ser distribuído como um pacote `.xapp`. O pacote reúne o manifesto e os arquivos necessários para identificar e executar o aplicativo.

A ferramenta de empacotamento pode validar a estrutura do projeto e incluir metadados adicionais, como informações de versão e permissões. Os detalhes de comandos, formatos internos e validações devem ser definidos pela implementação do Imagine Application Framework.

## 5. Compatibilidade

Cada aplicativo deve declarar metadados suficientes para que o sistema identifique seu nome, versão, ponto de entrada e janela solicitada. Requisitos adicionais de compatibilidade e permissões devem ser especificados pela versão correspondente do framework.
