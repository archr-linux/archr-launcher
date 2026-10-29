# Temas do archr-launcher

Um tema é uma pasta com um `theme.toml`. Ele pode trocar cores, fontes, ícones,
imagens de console e também **onde cada coisa fica** nas telas de sistemas e de
jogos. Tudo é opcional: o que o tema não diz vem do tema padrão (Grafite),
chave por chave.

## Onde ficam

```
/storage/.config/archr-launcher/themes/<nome>/   temas do usuário (têm prioridade)
/usr/share/archr-launcher/themes/<nome>/         temas do sistema
  theme.toml          obrigatório
  fonts/              fontes .ttf citadas no theme.toml (opcional)
  icons/              ícones Lucide .svg que substituem os embutidos (opcional)
  systems/            imagens de console: snes.png, gba.webp... (opcional)
  sounds/             sons da interface .ogg/.wav (opcional, ver Sons)
```

O tema aparece em Configurações › Aparência › Tema assim que a pasta existe.
Um exemplo completo e pequeno está em `themes/praia/`.

## Nome

```toml
name = "Praia"                                    # um nome para todos os idiomas
name = { en = "Beach", pt_BR = "Praia", ru = "Пляж" }   # ou por idioma
```

Sem `name`, a Aparência mostra o nome da pasta. Idiomas: `en`, `pt_BR`, `es`,
`ru`; o que faltar cai no `en` e depois em qualquer um.

## Cores

`#RRGGBB` ou `#RRGGBBAA`.

| chave | uso |
|---|---|
| `background` | fundo |
| `surface`, `surface_elevated` | painéis e janelas |
| `border`, `badge` | bordas e botões do rodapé |
| `text`, `text_secondary`, `muted`, `dim` | texto, do mais forte ao mais apagado |
| `accent`, `accent_text`, `accent_text_secondary` | destaque e o texto sobre ele |
| `success`, `warning` | estados positivos e de atenção |
| `scrim` | véu atrás de avisos e menus |

### Aparência por cima do tema

Em Aparência, a **1ª bolinha** da Cor de destaque é o `accent` do tema ativo,
com o `accent_text` do próprio tema. As seis cores prontas vêm depois (a que for
igual à do tema some) e a personalizada por último. Quando o usuário escolhe outra
cor, o texto sobre ela fica preto ou branco automaticamente, pelo contraste.

O Fundo começa em **Do tema** (as superfícies do tema); Preto e Azul-noite são
alternativas. Trocar de tema leva junto tudo que está em "do tema"; o que o
usuário escolheu explicitamente continua. **Restaurar padrão** volta tudo para
o tema escolhido.

## Fontes

```toml
font = "fonts/Inter-Regular.ttf"
font_semibold = "fonts/Inter-SemiBold.ttf"
font_mono = "fonts/JetBrainsMono-Regular.ttf"
font_mono_medium = "fonts/JetBrainsMono-Medium.ttf"
```

Caminhos relativos à pasta do tema, sem `..`. Sem fontes, o launcher usa as do
sistema (DejaVu/Liberation).

## Medidas gerais

Frações da tela: alturas, raios e tamanhos de fonte são frações da **altura**
(480 px de referência); larguras e `x`, da **largura** (640 px).

```toml
font_size = 0.035416667      # 17 px
margin = 0.033333333         # 16 px
radius_item = 0.020833333    # 10 px
radius_card = 0.033333333    # 16 px
header_height = 0.091666667  # 44 px
footer_height = 0.1          # 48 px
row_height = 0.079166667     # linhas de sistemas
selected_height = 0.091666667
game_row_height = 0.075      # linhas de jogos
game_selected_height = 0.0875
list_width = 0.590625        # largura da lista de jogos
cover = { x = 0.640625, y = 0.1125, w = 0.328125, h = 0.583333333 }
```

Essas medidas montam a visualização clássica, `lista`.

## Visualizações

Uma visualização posiciona os elementos das telas de sistemas (`systems`) e de
jogos (`games`). O usuário escolhe entre as que o tema declara em Aparência ›
Visualização (`launcher.view`). Toda visualização parte da `lista` e muda só o
que escreve; o tema herda as visualizações do padrão (hoje `lista` e `vitrine`)
e pode alterá-las ou criar novas.

```toml
[view.lado]
name = { en = "Side", pt_BR = "Lado" }

[view.lado.games]
list = { x = 0.4, w = 0.575 }          # lista à direita
art = { x = 0.025, w = 0.328125 }      # capa à esquerda
```

Elementos de cada tela (`[view.<id>.systems]` ou `[view.<id>.games]`):

| chave | o que é | campos |
|---|---|---|
| `header` | cabeçalho | `"full"` (nome e contador) ou `"compact"` (uma linha discreta: "SNES · 4 de 213") |
| `status` | pílula de relógio/bateria | `true`/`false` |
| `divider` | linha acima do rodapé | `true`/`false` |
| `summary` | resumo da biblioteca no rodapé (sistemas) | `true`/`false` |
| `list` | a lista | `x y w h`, `row`, `selected` (altura da linha escolhida), `gap`, `font`, `selected_font`, `radius`, `mark`, `scroll` |
| `panel` | cartão em relevo (painel do sistema, moldura da capa) | `x y w h`, `radius` |
| `art` | imagem do console ou capa do jogo | `x y w h`, `fit`, `radius` |
| `backdrop` | a capa ao fundo da tela de jogos | `opacity`, `solid` (fração da largura em fundo puro), `edge` (opacidade do fundo na borda direita) |
| `info` | nome do sistema e detalhes | `x y w h`, `title`, `meta` (tamanhos), `align` (`left`/`center`/`right`), `valign` (`top`/`bottom`), `details` (`stacked`: fabricante, nome, contagem; `inline`: nome e "213 jogos · Nintendo · 1990") |
| `meta` | ano, jogadores e "jogado há" sob a capa | `x y w h`, `size` |
| `dots` | pontos de posição entre sistemas | `x y w h` |

Valores de `list`:

- `mark`: `"pill"` (faixa colorida), `"bar"` (barrinha ao lado do texto) ou
  `"tile"` (sistemas em blocos lado a lado, o escolhido grande no centro, com a
  imagem do console; só na tela de sistemas). Num carrossel de blocos, ← e →
  (e os ombros, que o launcher lê como os mesmos botões) andam um sistema;
  ↑ e ↓ também andam um, para nenhum botão saltar ao fim. Numa lista vertical,
  ↑/↓ andam uma linha e ←/→ uma página, do tamanho que a lista mostra
  (`h` dividido por `row + gap`).
- `scroll`: `"edge"` (rola ao chegar na borda) ou `"center"` (mantém a escolha no meio).

Todas as linhas não escolhidas usam uma cor só: `text_secondary` (coleções em
`muted`). Mover a escolha nunca muda a cor das outras linhas.

Um elemento opcional some com `false` (`panel = false`) e aparece com uma
tabela. Valores fora da faixa são limitados à tela, chaves desconhecidas e tipos
errados são ignorados com aviso no log; nada disso derruba o launcher.

### Criar uma visualização nova

1. Copie `themes/praia/` para `/storage/.config/archr-launcher/themes/meu/`.
2. Acrescente `[view.minha]` com um `name` e os elementos que mudam.
3. Em Aparência, escolha o tema **meu** e a visualização **minha**; ao sair da
   página a mudança vale na hora, sem reiniciar.

## Detalhes do jogo

A tela de detalhes (Y › Detalhes, na lista de jogos) põe cada bloco num
retângulo do `theme.toml`. Um tema pode mover, redimensionar ou trocar os
blocos de lado; cada chave é opcional e herda sozinha do tema padrão. As
chaves `details_*` ficam no topo do `theme.toml` e valem para todas as
visualizações.

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

## Animações

Aparência › Animações (`launcher.animations`): **Ligadas** (`on`), **Reduzidas**
(`reduced`: só fades curtos, nada se desloca nem cresce) e **Desligadas**
(`off`: tudo instantâneo). Os valores antigos `1` e `0` valem como Ligadas e
Desligadas. Os tempos são fixos: seleção 180 ms, sistemas→jogos 220 ms, troca de
capa 200 ms, menus 160 ms, avisos 200 ms, interruptores 150 ms, abertura 300 ms.
Qualquer botão encerra a animação em curso; a entrada nunca espera.

## Sons

Um tema pode ter sons: de movimentação, de ação e de cada sistema. Tudo é
dado do tema; nenhum som é exigido, e um tema sem sons fica em silêncio.

### O jeito mais simples

Crie a pasta `sounds/` no tema e coloque arquivos `.ogg` ou `.wav` com o nome
do evento. Não é preciso declarar nada no `theme.toml`.

| Arquivo        | Toca quando                                   |
|----------------|-----------------------------------------------|
| `boot`         | o launcher abre                               |
| `move`         | o cursor muda de item                         |
| `page`         | pula página (L/R) ou letra                    |
| `select`       | abre um sistema, página ou item (A)           |
| `back`         | volta (B)                                     |
| `launch`       | abre um jogo, ferramenta ou stream            |
| `menu_open`    | abre o menu rápido (Y)                        |
| `menu_close`   | fecha o menu rápido                           |
| `toggle`       | muda um valor nas configurações               |
| `error`        | uma operação falha                            |
| `notice`       | aparece um aviso                              |
| `favorite`     | marca ou desmarca favorito                    |

Evento sem arquivo fica em silêncio.

### Sons por sistema

`sounds/systems/<nome-do-sistema>.ogg` (ou `.wav`) toca ao parar o cursor
naquele sistema, por exemplo `sounds/systems/snes.ogg`. O som só toca depois
de uns 350 ms parado, então rolar a lista rápido não dispara nada. O sistema
mostrado ao abrir o launcher não toca.

### Tabela `[sounds]` (opcional)

Só é necessária para trocar o nome de um arquivo ou silenciar um evento que
tem arquivo. Os caminhos são relativos ao tema e não saem dele.

```toml
[sounds]
move = "sounds/tick.wav"   # outro arquivo
select = ""                # silêncio, mesmo existindo sounds/select.ogg

[sounds.systems]
snes = "sounds/jingle.ogg"
nes = ""
```

### Limites e formato

- WAV ou OGG Vorbis, curtos (até 1 MiB por arquivo); mono ou estéreo.
- Todos os sons são carregados na abertura, fora da thread de desenho.
- O volume (0 a 100%) e o interruptor ficam em Configurações > Som e tela
  ("Sons do tema" e "Volume dos sons"); o volume é relativo ao do sistema.
- O launcher libera a saída de áudio antes de abrir um jogo ou ferramenta,
  e a reabre ao voltar.
- O tema `default` traz um conjunto sintetizado (ver `themes/default/sounds/README.md`)
  e nenhum som por sistema.

## Testar no PC

Captura das telas de sistemas, jogos (com e sem capa), Detalhes do jogo e
Aparência, em pt-BR e ru, para cada `pasta:visualização`:

```sh
ARCHR_SETTINGS_CAPTURE_DIR=/tmp/capturas \
ARCHR_CAPTURE_VIEWS=default:vitrine,praia:lista \
cargo test -p archr-ui --lib capture_views -- --ignored
```

O tema precisa estar em `themes/` do repositório para esse comando. No
aparelho, os avisos sobre chaves inválidas ficam no log do launcher.
