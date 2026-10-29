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
Pasta que o tema não traz vem do tema padrão instalado
(`/usr/share/archr-launcher/themes/default/`): sem `fonts/` e sem chaves de
fonte, ele usa as fontes do padrão; sem a imagem de um console, a do padrão;
sem um ícone, o do padrão; sem um som, o do padrão.
Um exemplo completo e pequeno está em `themes/praia/` (aparece como "Exemplo";
o tema Praia completo é publicado à parte, na Loja de temas).

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

Caminhos relativos à pasta do tema, sem `..`. Cada família vem inteira: o tema
que cita `font` ou `font_semibold` usa as suas (a regular serve de seminegrito
se faltar a outra); o que não cita nenhuma das duas herda as duas do tema
padrão. O mesmo vale para `font_mono` e `font_mono_medium`. Uma fonte citada
que não existe cai nas do sistema (DejaVu/Liberation).

### Tamanho e nitidez do texto

```toml
font_scale = 1.4        # 0,5 a 2: multiplica todo tamanho de texto (com o do usuário)
font_grid = 8           # 0 a 32 px: tamanhos ficam no múltiplo mais próximo (0 = livre)
font_smoothing = false  # sem suavização: letras de borda dura, para fontes pixel
font_min = 12           # 6 a 32 px: nenhum texto fica menor; o que não cabe quebra linha
```

`font_grid` serve a fontes desenhadas numa grade (Press Start 2P fica nítida
em múltiplos de 8 px). Vale também nas Configurações, onde o tema só muda
cores, fontes e ícones.

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

## Estilo

Cinco chaves no topo do `theme.toml` mudam o acabamento de todas as telas,
inclusive as Configurações. Cada uma herda do padrão sozinha; valor inválido
volta ao padrão com aviso no log.

| chave | valores | padrão | efeito |
|---|---|---|---|
| `corners` | 0 a 1 | `1.0` | multiplica todo raio de canto; `0` deixa tudo reto |
| `border_width` | 1 a 8 | `1` | espessura das bordas, em px de referência (480 px de altura) |
| `shadow` | `"soft"`, `"hard"`, `"none"` | `"soft"` | sombra dos painéis: esfumada, bloco preto deslocado 4 px, ou nenhuma (`"soft"` também põe a sombra de contato sob a imagem do console) |
| `art_scaling` | `"smooth"`, `"pixel"` | `"smooth"` | `"pixel"` amplia as imagens de console em múltiplos inteiros e sem suavizar (pixel art nítida); capas de jogos não mudam |
| `easing` | `"smooth"`, `"steps"` | `"smooth"` | `"steps"` troca a desaceleração por quatro saltos iguais, o primeiro na hora (movimento "8 bits") |

```toml
corners = 0
border_width = 3
shadow = "hard"
art_scaling = "pixel"
easing = "steps"
```

## Imagens do tema

Nas telas de sistemas, jogos e detalhes (nunca nas Configurações) o tema pode
trocar caixas lisas por imagens. Tudo é opcional; o que não existe é desenhado
como antes. As imagens ficam na pasta do tema (`.png`, `.jpg`) e carregam fora
da thread de desenho: no primeiro quadro pode aparecer a caixa lisa.

### Fundo

```toml
background_image = "images/fundo.png"   # cobre a tela inteira, cortado no centro
```

Com `art_scaling = "pixel"` a imagem é ampliada sem suavizar (desenhe-a em
320×240 para pixels 2×2).

### Molduras (9-slice)

```toml
[frames.row]
image = "frames/linha.png"
slice = [6, 5, 6, 7]   # px da imagem mantidos inteiros: esquerda, topo, direita, base (ou um número)
scale = 2              # px de tela (referência 480) por px da imagem; 0,25 a 8
pad = 0                # quanto a moldura passa da caixa, em px de referência (ou [e, t, d, b])
tile = false           # true: bordas e meio se repetem em vez de esticar
```

Os cantos ficam inteiros, as bordas e o meio esticam (ou se repetem com
`tile`). Caixas que aceitam moldura:

| nome | onde |
|---|---|
| `header`, `footer` | faixa do cabeçalho e do rodapé (altura de `header_height`/`footer_height`) |
| `list` | atrás da lista (`list`), use `pad` para a moldura sobrar |
| `row`, `selected` | cada linha da lista e a linha escolhida (no lugar da faixa colorida) |
| `panel` | painel do sistema, menus e, nos detalhes, a tabela e a coluna do título aos botões |
| `art` | moldura da capa e da tela do jogo nos detalhes |
| `button`, `button_selected` | botões dos detalhes |
| `badge` | botões A/B/X/Y do rodapé (a letra vem na cor `text`) |
| `tile`, `tile_selected` | blocos da visualização em blocos (`mark = "tile"`) |

O texto sobre as molduras usa as cores de sempre: `text_secondary` nas
linhas, `accent_text` na escolhida, `text` e `muted` nos painéis.

### Contorno do texto

```toml
text_outline = { color = "#3F2214", width = 2, ink = "#FFFFFF" }
```

Vale para o texto desenhado direto sobre o fundo: cabeçalho, dicas do rodapé
e o nome do sistema quando a visualização não tem painel (`panel = false`).
`width` vai de 0 a 4 px de referência; `ink` (opcional) troca a cor desse
texto, para letras claras com contorno escuro sobre um fundo de imagem.

### Decorações

Cada tela de cada visualização pode ter imagens soltas, paradas ou animadas:

```toml
[view.lista.systems]
decor = [
  { image = "decor/palmeira.png", x = 0.88, y = 0.02, scale = 2 },
  { image = "decor/onda.png", x = 0.0, y = 0.82, scale = 2, layer = "back" },
  { image = "decor/caranguejo.png", x = 0.02, y = 0.8, scale = 2, frames = 2, fps = 2 },
]
```

`x`, `y`: canto superior esquerdo em frações da tela. `scale`: px de
referência por px da imagem. `frames`: quadros lado a lado na imagem,
trocados `fps` vezes por segundo (parados com Animações desligadas).
`layer = "back"` desenha atrás da lista e dos painéis; o padrão é na frente.
A lista substitui a herdada; `decor = []` limpa. A tela de detalhes usa as
decorações da tela de jogos.

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
| `title` | título do cabeçalho de sistemas | `"brand"` (ARCHR espaçado, o padrão) ou `"screen"` ("Sistemas", no idioma) |
| `status` | pílula de relógio/bateria | `true`/`false` |
| `divider` | linha acima do rodapé | `true`/`false` |
| `summary` | resumo da biblioteca no rodapé (sistemas) | `true`/`false` |
| `list` | a lista | `x y w h`, `row`, `selected` (altura da linha escolhida), `gap`, `font`, `selected_font`, `radius`, `mark`, `scroll`, `cursor` |
| `panel` | cartão em relevo (painel do sistema, moldura da capa) | `x y w h`, `radius` |
| `art` | imagem do console ou capa do jogo | `x y w h`, `fit`, `radius` |
| `backdrop` | a capa ao fundo da tela de jogos | `opacity`, `solid` (fração da largura em fundo puro), `edge` (opacidade do fundo na borda direita) |
| `info` | nome do sistema e detalhes | `x y w h`, `title`, `meta` (tamanhos), `align` (`left`/`center`/`right`), `valign` (`top`/`bottom`), `details` (`stacked`: fabricante, nome, contagem; `inline`: nome e "213 jogos · Nintendo · 1990") |
| `meta` | ano, jogadores e "jogado há" sob a capa | `x y w h`, `size` |
| `dots` | pontos de posição entre sistemas | `x y w h` |
| `decor` | imagens soltas (ver [Decorações](#decorações)) | lista de `{ image, x, y, scale, frames, fps, layer }` |

Valores de `list`:

- `mark`: `"pill"` (faixa colorida), `"bar"` (barrinha ao lado do texto) ou
  `"tile"` (sistemas em blocos lado a lado, o escolhido grande no centro, com a
  imagem do console; só na tela de sistemas). Num carrossel de blocos, ← e →
  (e os ombros, que o launcher lê como os mesmos botões) andam um sistema;
  ↑ e ↓ também andam um, para nenhum botão saltar ao fim. Numa lista vertical,
  ↑/↓ andam uma linha e ←/→ uma página, do tamanho que a lista mostra
  (`h` dividido por `row + gap`).
- `scroll`: `"edge"` (rola ao chegar na borda) ou `"center"` (mantém a escolha no meio).
- `cursor`: texto curto (até 4 caracteres, na fonte da lista) desenhado antes
  do item escolhido, piscando meio segundo aceso e meio apagado, por exemplo
  `cursor = "▶"` (a fonte precisa ter o caractere). Com Animações desligadas
  fica aceso. Vazio (o padrão) não desenha nada. Não vale para blocos (`tile`).

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

## Conquistas do jogo

A tela de conquistas (Y › Conquistas, na lista de jogos, com o
RetroAchievements ligado e uma conta conectada) também segue retângulos do
`theme.toml`, no topo do arquivo, com a mesma regra dos `details_*`: frações
da tela, herança do tema padrão e volta ao padrão quando o valor é inválido.

| chave | conteúdo |
|---|---|
| `achievements_progress` | "Progresso", contagem e pontos sobre a barra; o selo Hardcore fica na ponta direita |
| `achievements_list` | lista das conquistas, linhas de 50 px (referência 480 px) com 2 px entre elas; L R vira a página |

O padrão (maquete RA-jogo):

```toml
achievements_progress = { x = 0.025, y = 0.108333333, w = 0.94375, h = 0.0625 }
achievements_list = { x = 0.025, y = 0.183333333, w = 0.94375, h = 0.7125 }
```

Cabem na página quantas linhas couberem na altura de `achievements_list`.
As cores vêm das chaves gerais: `surface` e `accent` na linha selecionada,
`accent` na barra e no selo, `border` no trilho da barra, `badge` no ícone
de conquista bloqueada sem imagem, `muted` nas descrições e nos pontos.
Conquistas bloqueadas aparecem esmaecidas. Títulos e descrições vêm do
RetroAchievements: o título encolhe até 12 px e a descrição quebra em duas
linhas menores antes de ser cortada.
## Animações

Aparência › Animações (`launcher.animations`): **Ligadas** (`on`), **Reduzidas**
(`reduced`: só fades curtos, nada se desloca nem cresce) e **Desligadas**
(`off`: tudo instantâneo). Os valores antigos `1` e `0` valem como Ligadas e
Desligadas. Os tempos são fixos: seleção 180 ms, sistemas→jogos 220 ms, troca de
capa 200 ms, menus 160 ms, avisos 200 ms, interruptores 150 ms, abertura 300 ms.
O tema escolhe só a curva (`easing`, ver [Estilo](#estilo)).
Qualquer botão encerra a animação em curso; a entrada nunca espera.

## Sons

Um tema pode ter sons: de movimentação, de ação e de cada sistema. Tudo é
dado do tema; nenhum som é exigido: o que faltar vem do tema padrão.

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

Evento sem arquivo toca o som do tema padrão; para silenciar, use `""` na
tabela `[sounds]` (abaixo).

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

## Publicar na Loja de temas

Configurações › Aparência › **Loja de temas** procura temas no GitHub, mostra
capa, detalhes e imagens, e instala com um botão. A loja só enxerga
repositórios que seguem as regras abaixo; o resto do GitHub não aparece.

### O que a loja procura

1. **Repositório público** no GitHub, não arquivado.
2. Com o **tópico `archr-launcher-theme`** (na página do repositório:
   engrenagem ao lado de "About" › Topics).
3. A **raiz do repositório é a pasta do tema**: `theme.toml` fica na raiz,
   não numa subpasta. Um repositório = um tema.
4. `theme.toml` com `name` (ver [Nome](#nome)). Sem `name`, ou com um
   `theme.toml` que não é TOML válido, o tema aparece na lista mas não instala.
5. **`preview.png`** na raiz: 640×480, PNG, até 2 MB. É a capa na grade e a
   primeira imagem dos detalhes. Na grade aparece recortada no meio
   (faixa larga e baixa), então deixe o centro da imagem representativo: uma
   captura da tela de sistemas ou de jogos com o tema funciona bem.
6. Opcional: **`screenshots/1.png`**, `2.png` e `3.png`, 640×480, até 2 MB
   cada. Aparecem nos detalhes, trocadas com L/R.

Nomes de dono, repositório e tag: letras, números, `.`, `_` e `-` (sem `/`).
A loja mostra os 30 primeiros resultados da busca, por estrelas (Populares)
ou por atualização (Recentes).

### Metadados no `theme.toml`

Todos opcionais; os detalhes mostram o que houver.

```toml
name = { en = "Neon Arcade", pt_BR = "Neon Arcade" }
description = { en = "Neon pink over deep purple.", pt_BR = "Rosa neon sobre roxo profundo." }
author = "kaio-dev"          # sem autor, aparece o dono do repositório
version = "1.2.0"            # sem versão, aparece a tag da release
license = "CC BY 4.0"        # texto curto; o arquivo LICENSE também é instalado
min_launcher = "0.2"         # versão mínima do launcher; abaixo dela não instala
```

`description` aceita texto ou tabela por idioma, como `name`, até 200
caracteres. "Inclui" nos detalhes é calculado pelos arquivos: cores sempre;
fontes (`.ttf`/`.otf`), layout (`[view.*]`), artes (`systems/`), ícones
(`icons/`) e sons (`sounds/`) quando existem. "Tamanho" soma só o que é
instalado.

### Qual versão é instalada

- Com **release** no GitHub: a mais recente ("Latest release"), pela tag.
- Sem release: o **branch padrão** como está.

A loja guarda o identificador do conteúdo instalado. Quando a release (ou o
branch) muda, o botão dos detalhes passa de **Usar agora** para
**Atualizar**. Para soltar uma versão nova, publique uma release nova.

### O que é instalado

O tarball da versão é baixado de `codeload.github.com` e só estes arquivos
são copiados:

- extensões `toml`, `png`, `jpg`, `jpeg`, `svg`, `ttf`, `otf`, `ogg`, `wav`,
  `txt`, `md`;
- `LICENSE`, `LICENCE` e `COPYING`.

Qualquer outro arquivo é ignorado, assim como arquivos e pastas que começam
com `.` (`.github/`, `.gitignore`), links simbólicos, hard links e arquivos
especiais. Um caminho com `..` ou absoluto recusa o tema inteiro. Nada é
executado.

Limites: download até **25 MB**, conteúdo instalado até **60 MB**, até
5000 entradas no arquivo e 8 níveis de pasta. Lembre que sons do tema têm
limite próprio de 1 MiB cada (ver [Sons](#sons)).

O tema vai para `/storage/.config/archr-launcher/themes/<dono>-<repositório>/`
com um arquivo `.archr-store.toml` (de onde veio, qual versão). A troca é
atômica: até terminar, a versão anterior continua lá. A loja nunca escreve
por cima de uma pasta que ela não criou, e só remove temas que ela instalou;
o tema em uso não pode ser removido. Instalar não ativa: depois de instalar,
**Usar agora** escolhe o tema na Aparência, que grava ao sair.

### Exemplo de repositório

```
github.com/kaio-dev/archr-neon-arcade     tópico: archr-launcher-theme
├── theme.toml          name, description, author, version, license
├── preview.png         640×480
├── screenshots/
│   ├── 1.png
│   └── 2.png
├── fonts/
│   └── Orbitron-Regular.ttf
├── systems/
│   └── snes.png
├── sounds/
│   ├── move.ogg
│   └── select.ogg
├── LICENSE
└── README.md
```

Teste antes de publicar copiando a pasta para
`/storage/.config/archr-launcher/themes/` no aparelho; o que funciona ali
funciona instalado pela loja.

### Rede e limites do GitHub

A loja usa a API do GitHub sem login, que permite poucas consultas por hora.
Listas e detalhes ficam guardados por uma hora (Y força uma nova busca);
quando o GitHub limita, a lista guardada continua aparecendo com um aviso.
Sem rede, a aba **Instalados** funciona e mostra os detalhes de cada tema a
partir da pasta instalada.

## Testar no PC

Captura das telas de sistemas, jogos (com e sem capa), Detalhes do jogo e
Aparência, em pt-BR e ru, para cada `pasta:visualização`:

```sh
ARCHR_SETTINGS_CAPTURE_DIR=/tmp/capturas \
ARCHR_CAPTURE_VIEWS=default:vitrine,praia:lista,praia:vitrine:noart \
cargo test -p archr-ui --lib capture_views -- --ignored
```

`:noart` captura como se não houvesse imagem de console nenhuma. A pasta é
relativa a `themes/` do repositório, ou um caminho absoluto (um tema que vive
no próprio repositório, como os da Loja). No
aparelho, os avisos sobre chaves inválidas ficam no log do launcher.
