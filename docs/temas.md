# Temas

## Detalhes do jogo

A tela de detalhes (Y › Detalhes, na lista de jogos) põe cada bloco num
retângulo do `theme.toml`. Um tema pode mover, redimensionar ou trocar os
blocos de lado; cada chave é opcional e herda sozinha do tema padrão.

Cada valor é `{ x, y, w, h }`: `x` e `w` são frações da largura da tela,
`y` e `h` frações da altura (referência 640×480). O retângulo precisa caber
na tela (`x + w <= 1`, `y + h <= 1`); um valor inválido volta ao padrão.

| chave | conteúdo |
|---|---|
| `details_shot` | tela do jogo (ou a capa, sem tela); com vídeo, mostra a tela parada |
| `details_info` | tabela gênero, jogadores, lançamento, desenvolvedora, editora e nota |
| `details_title` | logo do jogo, alinhado à esquerda; sem logo, o nome em texto |
| `details_subtitle` | sistema · última vez jogado |
| `details_text` | descrição, rolada com ↑ ↓ |
| `details_actions` | botões Jogar, Favorito e Coletar de novo |

O padrão (maquete Jogo-detalhes):

```toml
details_shot = { x = 0.0375, y = 0.104166667, w = 0.390625, h = 0.3125 }
details_info = { x = 0.0375, y = 0.4375, w = 0.390625, h = 0.445833333 }
details_title = { x = 0.45625, y = 0.104166667, w = 0.50625, h = 0.095833333 }
details_subtitle = { x = 0.45625, y = 0.216666667, w = 0.50625, h = 0.033333333 }
details_text = { x = 0.45625, y = 0.266666667, w = 0.50625, h = 0.529166667 }
details_actions = { x = 0.45625, y = 0.8125, w = 0.50625, h = 0.070833333 }
```

As cores vêm das chaves gerais (`surface`, `border`, `text_secondary`,
`muted`, `accent`, `warning` para as estrelas). Linhas da tabela que não
cabem em `details_info` são omitidas; texto nunca é cortado com reticências:
encolhe e quebra linha.
