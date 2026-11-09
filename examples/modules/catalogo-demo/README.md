# Catálogo de demonstração

Módulo de exemplo do [archr-launcher](https://github.com/archr-linux/archr-launcher)
para a API 4: um app com telas próprias, aberto por um item no carrossel. O
catálogo é local (quatro cartuchos de teste com capas que vêm no pacote); nada
vem da rede.

- Tipo: `daemon`, sh + `jq`, API 4, com `[app]`. O processo só roda com o app
  aberto (`ARCHR_MODULE_ROLE=app`).
- Permissões: `install = ["gb"]` e `launch`. "Baixar" cria um arquivo de teste
  de 32 KB de zeros em `HOME/downloads/` e pede `rom.install`; o launcher o
  copia para a pasta do Game Boy com capa e entrada no `gamelist.xml`.
  "Remover" pergunta com `screen.confirm` e só então pede `rom.remove`.
- Telas: catálogo (`collection`, chips Todos/Instalados, três itens e o quarto
  com `screen.more`, busca pelo SELECT) e detalhe (`detail`, fatos, texto,
  botões com ícone e barra de progresso).

Teste no PC, sem o launcher: `sh examples/modules/catalogo-demo.test.sh`.

*An API 4 example: an app with its own screens, a local catalog of test
cartridges it installs in Game Boy through the launcher. sh + jq, no network.*
