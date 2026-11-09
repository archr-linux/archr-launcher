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
| `success`, `warning`, `danger` | estados positivos, de atenção e de falha |
| `scrim` | véu atrás de avisos e menus |

O aviso de partição das ROMs com erro (só leitura ou não montada) fica no
rodapé das listas de sistemas e de jogos, no lugar do resumo, com o ícone
`icons/triangle-alert.svg` e a cor `warning`; o layout não muda.

### Aparência por cima do tema

Em Aparência, a **1ª bolinha** da Cor de destaque é o `accent` do tema ativo,
com o `accent_text` do próprio tema. As seis cores prontas vêm depois (a que for
igual à do tema some) e a personalizada por último. Quando o usuário escolhe outra
cor, o texto sobre ela fica preto ou branco automaticamente, pelo contraste.

O Fundo começa em **Do tema** (as superfícies do tema); Preto, Azul-noite e uma cor personalizada são
alternativas. Trocar de tema leva junto tudo que está em "do tema"; o que o
usuário escolheu explicitamente continua. **Restaurar padrão** volta tudo para
o tema escolhido.

### Opções do usuário que agem sobre o tema

Em Aparência há escolhas gravadas no `system.cfg` que valem por cima de **qualquer** tema. Nenhuma é chave do `theme.toml`: o tema não as declara nem as desliga, e elas se aplicam depois de ele ser lido.

| opção (`system.cfg`) | valores | efeito |
|---|---|---|
| Peso da fonte (`launcher.font_weight`) | do tema (vazio) / `bold` | `bold` usa a `font_semibold` do tema (com a fonte mono, a `font_mono_medium`). Tema sem essa face continua na regular. |
| Porcentagem da bateria (`launcher.battery_percent`) | ligada (padrão) / desligada | desligada esconde o número na pílula de status; o ícone fica. |
| Capa do jogo (`launcher.game_art`) | ligada (padrão) / desligada | desligada remove da tela de jogos `art`, `meta`, `backdrop` e o `panel` que coincide com a capa. |
| Largura da capa (`launcher.art_width`) | do tema / 35, 40, 45, 50, 55 % da largura da tela | a capa mantém a borda de fora (a que fica longe da lista); a lista ao lado mantém a folga até ela; `meta` acompanha a capa. |
| Cantos da capa (`launcher.art_radius`) | do tema / 0, 6, 12, 20 px de referência | substitui o `radius` da `art` da visualização; o `panel` acompanha. |
| Fundo do alternador (`launcher.switcher_curtain`) | fundo do tema / 40, 60, 80 % | a imagem do jogo selecionado preenche a tela atrás do alternador, sob uma cortina da cor `background` do tema nessa opacidade. Jogo sem imagem fica com o fundo do tema. |
| Cor do texto (`launcher.text_color`) | do tema (vazio) / `#RRGGBB` | substitui o `text` do tema; o `text_secondary` é derivado (25 % em direção ao `background`). |
| Cor do texto selecionado (`launcher.selected_text_color`) | do tema (vazio) / `#RRGGBB` | substitui o `highlight_text` do tema e o `accent_text_secondary` que o acompanha. |
| Cor do texto secundário (`launcher.secondary_color`) | do tema (vazio) / `#RRGGBB` | substitui o `muted` do tema e o `dim_text` que o acompanha. |
| Fundo (`launcher.background`) | do tema (vazio) / `black` / `midnight` / `#RRGGBB` | com `#RRGGBB`, vira o `background`; `surface`, `surface_elevated`, `border` e `badge` são derivados misturando em direção ao branco (4, 7, 12 e 12 %). |
| Dicas só com texto (`launcher.text_hints`) | desligada (padrão) / ligada | as dicas de botão do rodapé mostram só a letra do botão na cor `text` do tema, sem o círculo `badge` nem a moldura `badge`; o START mantém a palavra, sem a caixa. |
| Fundo por sistema (`launcher.system_background`) | ligada (padrão) / desligada | na lista de jogos e nos detalhes de um sistema, o fundo da tela é a imagem desse sistema (veja "Fundos por sistema"). |

Detalhes que importam a quem faz tema:

- **Cores**: cada uma das três cores de texto abre o editor de cor (matiz, saturação, brilho e código hex). Vazio mantém o tema. Como substituem `text`, `highlight_text` e `muted`, escolha no tema valores que ainda funcionem com um texto de outra cor: não dependa de a cor ser exatamente a sua.
- **Dicas só com texto**: o tema não precisa fazer nada; o texto usa o `text` do tema, então ele deve ter contraste com o `background`.
- **Fundo por sistema**: é um recurso que o tema pode usar (próxima seção, "Fundos por sistema"); a opção só liga ou desliga.
- **Capa desligada**: uma lista ao lado da capa, à esquerda ou à direita, passa a ocupar toda a largura, entre margens iguais (a margem da lista do lado oposto à capa, repetida dos dois lados). O `panel` só some junto se tiver o mesmo `x` e `w` da `art`; um `panel` com outro retângulo (um cartão maior, uma moldura de enfeite) continua desenhado.
- **Capa mais larga**: o retângulo `art` é refeito a partir da borda de fora que o tema escolheu (a direita, se a capa está à direita da lista; a esquerda, se está à esquerda), e a lista ao lado recua, mantendo a folga original e a sua outra borda. Uma lista que não está ao lado da capa (acima ou abaixo dela) não é mexida.
- **Cantos**: valem para a `art` e, se o `panel` coincide com ela, para o `panel` (1 px de referência a mais, para a moldura envolver a capa).
- **Alternador**: a cortina usa o `background` do tema; escolha um `background` que combine com capas claras e escuras.

Para o tema continuar bom com essas escolhas: ponha a lista ao lado da capa; mantenha `art` e `panel` iguais quando o painel é o cartão da capa; não dependa de um tamanho exato de capa nem de a capa existir (o título e as informações precisam funcionar sozinhos). Teste o tema com a capa desligada, mais larga (55 %), com cantos 0 e 20, com peso Negrito e com a cortina do alternador em 40 e 80 %. Teste também com as dicas só com texto, com cores de texto e de fundo personalizadas e com um fundo por sistema.

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
                        # (rótulos dos blocos do Menu Start podem ficar 2 px abaixo, nunca abaixo de 8)
```

`font_grid` serve a fontes desenhadas numa grade (Press Start 2P fica nítida
em múltiplos de 8 px). Vale também nas Configurações, onde o tema só muda
cores, fontes e ícones. Padrões: `1.0`, `0`, `true` e `8`.

`font_min` e `font_grid` são px de referência (480 px de altura) e mudam
com a tela. Por cima deles vale um piso fixo de **10 px de tela**: nenhum
texto é desenhado menor que isso, em qualquer resolução, com qualquer
`font_scale`, `font_min` ou tamanho de texto escolhido pelo usuário (veja
[Telas de outros tamanhos](#telas-de-outros-tamanhos)).

## Medidas gerais

Há dois tipos de número. **Tamanhos** (fontes, alturas de linha, de
cabeçalho e de rodapé, raios, `margin`) são frações de **480 px de
referência**: `0.035416667` é 17 px numa tela de 640×480 e cresce ou
encolhe com a tela, como todo o resto. **Retângulos** (`x y w h`) são
frações da **tela**: `x` e `w` da largura, `y` e `h` da altura. Numa tela
de outro formato os retângulos esticam com ela e os tamanhos não, então
cabem mais linhas numa tela alta; veja [Telas de outros
tamanhos](#telas-de-outros-tamanhos).

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

## Telas de outros tamanhos

O launcher roda em 640×480 (4:3: R36S e a maioria dos portáteis RK3326),
720×720 (1:1, R36 Plus) e 480×320 (3:2: Odroid Go Advance, RGB10, RG351M;
o painel é retrato e o Sway o gira, então o launcher já recebe a tela
deitada). Uma tela 16:9 (854×480, 1280×720) cai no mesmo formato da de 3:2.

O desenho de 640×480 é ampliado ou reduzido até caber inteiro: a escala é a
menor entre largura/640 e altura/480, e o texto fica do mesmo tamanho
físico em telas da mesma densidade. A sobra vai para o lado mais longo, em
px de referência:

| tela | escala | tela em px de referência | formato |
|---|---|---|---|
| 640×480 | 1 | 640×480 | `classic` |
| 720×720 | 1,125 | 640×640 | `square` |
| 480×320 | 0,667 | 720×480 | `wide` |
| 1280×720 | 1,5 | 853×480 | `wide` |

O que isso faz com cada número do tema:

- **Tamanhos** (fontes, alturas de linha, cabeçalho, rodapé, raios,
  `margin`) não mudam em px de referência. Numa tela quadrada a lista de
  jogos tem linhas do mesmo tamanho e mostra mais delas (pelo menos 13 em
  vez de 10 no Grafite); L/R viram a página pelas linhas que a lista mostra.
- **Retângulos** (`cover`, `details_*`, `switcher_*`, `x y w h` das
  visualizações) são frações da tela e esticam com ela. Uma capa que é
  retrato em 640×480 fica mais alta e estreita em 720×720.
- A geometria embutida da `lista` (painel do sistema com 250 px de largura,
  margens, folgas sob o cabeçalho) é calculada em px de referência para a
  tela de verdade; ela não estica.

### Tabelas `[aspect.<formato>]`

Para mudar o layout de um formato, use uma tabela `[aspect.<formato>]`. O
formato sai da razão largura/altura da tela:

| formato | razão | telas |
|---|---|---|
| `square` | menor que 1,2 (6:5) | 720×720 |
| `classic` | de 1,2 a 1,45 (29:20), inclusive | 640×480, 800×600 |
| `wide` | maior que 1,45 | 480×320 (3:2), 854×480 e 1280×720 (16:9) |

Outro nome (`[aspect.tall]`) é ignorado com aviso no log.

Tudo que vale no topo do `theme.toml` vale dentro dela, inclusive
`[view.*]`, `frames`, `background_image` e `text_outline`, e ela se aplica
por cima do próprio arquivo, chave por chave: tabelas (visualizações,
`list`, `art`...) se misturam campo a campo; outro valor substitui. Um
retângulo do topo (`cover`, `details_*`, `switcher_*`, `start_area`) vem
inteiro, com os quatro campos.

A ordem é: tema padrão, `[aspect]` do padrão, o seu tema, o `[aspect]` do
seu tema. Duas consequências:

- Um tema sem nenhuma tabela `[aspect]` herda as do Grafite: num aparelho
  quadrado ele já ganha o Alternador em 4:3, o Menu Start centralizado e a
  capa mais larga.
- O que o seu tema escreve no topo vale em todos os formatos e passa por
  cima do `[aspect]` do padrão. Se o tema muda `cover` ou `list_width` no
  topo, a capa e a lista do Grafite para telas quadradas deixam de valer;
  escreva o seu `[aspect.square]` para elas.

```toml
# Num aparelho quadrado, a lista de jogos mais estreita e a capa mais alta.
[aspect.square.view.lista.games]
list = { w = 0.5 }
art = { x = 0.55, w = 0.4, h = 0.7 }

[aspect.square]
switcher_card = { x = 0.1875, y = 0.1, w = 0.625, h = 0.46875 }
```

Como toda tabela TOML, `[aspect.square]` vem depois das chaves do topo; uma
chave do topo escrita depois dela passa a valer só no formato quadrado.

O que não muda por formato: os arquivos de fonte (`font`,
`font_semibold`, `font_mono`, `font_mono_medium`), o `name`, os sons, os
ícones e as imagens de console. `font_size`, `font_scale` e `font_min`
mudam normalmente.

O tema padrão usa:

- `[aspect.square]`: Alternador e retomada em 4:3, Menu Start centralizado
  entre cabeçalho e rodapé (`start_area`), lista de jogos mais estreita e
  capa mais larga (`list_width`, `cover`), imagem e tabela dos detalhes
  dividindo a coluna, imagem do console no meio do painel e, na `vitrine`,
  capa mais larga;
- `[aspect.wide]`: listas em letra maior, 19 px de referência
  (`font_size = 0.039583333`, 13 px numa tela de 480×320), e a lista de
  jogos da `vitrine` em 18 px.

O tema Praia de exemplo repete a mudança da capa para o seu lado esquerdo
em `[aspect.square.view.lista.games]`.

### Texto: o piso de 10 px

Nenhum texto fica com menos de **10 px na tela**, em qualquer resolução. O
piso é absoluto: vale por cima de `font_min`, `font_scale` e do tamanho de
texto do usuário, e não é configurável. Numa tela pequena o texto mantém
esse mínimo e o resto do desenho encolhe; o que não cabe quebra linha ou
termina em reticências nas Configurações. Em 480×320 isso equivale a 15 px
de referência: um tamanho menor que esse (o padrão das linhas de apoio, 12
px) é desenhado com 10 px de tela. A única exceção é a miniatura do
MangoHud nas Configurações, que encolhe como imagem.

O teto é 128 px de tela.

### Menu Start: `start_area`

O Menu Start (título, blocos, atalhos e widgets) é uma composição de
640×480 px de referência. `start_area` (`{ x, y, w, h }`, frações da tela)
é a caixa onde ela fica: a composição tem 480 px de referência de altura,
a largura da caixa, e fica centralizada nela; os blocos podem ir até as
bordas da caixa. O rodapé fica sempre no fim da tela. O padrão é a tela
inteira; numa tela quadrada o Grafite a põe entre o cabeçalho e o rodapé
(0,75 de 640 px são os 480 px da composição):

```toml
[aspect.square]
start_area = { x = 0.0, y = 0.1796875, w = 1.0, h = 0.75 }
```

Uma caixa com menos de 480 px de referência de altura aperta a composição.
`start_widgets` (ver [Widgets de módulos no Menu
Start](#widgets-de-módulos-no-menu-start)) é medido dentro dessa
composição, não na tela.

### Configurações

As Configurações não mudam com o tema: têm 640 px de referência de largura,
centradas numa tela mais larga, e ganham linhas numa tela mais alta.

### Ver o tema em cada tamanho

No PC, as capturas de teste aceitam `ARCHR_CAPTURE_SIZE` (veja [Testar no
PC](#testar-no-pc)); o teste `resolutions` confere se o layout de cada tema
de `themes/` cabe nas três telas.

## Estilo

Cinco chaves no topo do `theme.toml` mudam o acabamento de todas as telas,
inclusive as Configurações. Cada uma herda do padrão sozinha; valor inválido
volta ao padrão com aviso no log.

| chave | valores | padrão | efeito |
|---|---|---|---|
| `corners` | 0 a 1 | `1.0` | multiplica todo raio de canto; `0` deixa tudo reto |
| `border_width` | 1 a 8 | `1` | espessura das bordas, em px de referência (480 px de altura) |
| `shadow` | `"soft"`, `"hard"`, `"none"` | `"soft"` | sombra dos painéis: esfumada, bloco preto deslocado 4 px, ou nenhuma (`"soft"` também põe a sombra de contato sob a imagem do console) |
| `art_scaling` | `"smooth"`, `"pixel"` | `"smooth"` | imagens de console: `"smooth"` reduz com filtro (para ilustrações, com sombras e bordas suaves); `"pixel"` amplia em múltiplos inteiros sem suavizar (pixel art em grade). Capas de jogos não mudam |
| `image_scaling` | `"smooth"`, `"pixel"` | `"smooth"` | o mesmo para as [imagens do tema](#imagens-do-tema): fundo, molduras e decorações |
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

Com `image_scaling = "pixel"` a imagem é ampliada sem suavizar (desenhe-a em
320×240 para pixels 2×2).

### Fundos por sistema

Na lista de jogos e nos detalhes de um sistema, o launcher usa a imagem do sistema como fundo da tela, se a opção Fundo por sistema (`launcher.system_background`, ligada por padrão) estiver ligada. O tema pode trazer essas imagens na pasta `backgrounds/`:

```
<tema>/backgrounds/snes.png
<tema>/backgrounds/gba.jpg
```

- O nome do arquivo é o id do sistema, como no `es_systems` (`snes`, `gba`, `psx`...), em `.png` ou `.jpg`.
- Ordem de busca: primeiro a pasta do usuário `/storage/.config/archr-launcher/backgrounds/`, depois o `backgrounds/` do tema, depois o `backgrounds/` do tema padrão.
- A imagem cobre a tela inteira (cortada para preencher) sob uma cortina da cor `background` do tema, a cerca de 59 % de opacidade (alfa 150), para o texto continuar legível. Ela substitui o `background_image` do tema nessas telas.
- As coleções (`@favorites`, `@recent`) mantêm o fundo do tema.
- A imagem é reduzida ao tamanho da tela quando carrega, então qualquer tamanho funciona; no tamanho da tela (640×480, 720×720 ou 1280×720) ela carrega mais rápido no aparelho.
- Sem imagem para o sistema, vale o fundo do tema. Escolha um `background` que combine com a cortina.

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
| `title` | título do cabeçalho de sistemas e do Menu Start | `"brand"` (ARCHR espaçado, o padrão) ou `"screen"` ("Sistemas"/"Menu Start", no idioma; o Menu Start então não repete o título no meio) |
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
`y` e `h` frações da altura (referência 640×480; outros formatos em
[Telas de outros tamanhos](#telas-de-outros-tamanhos)). O retângulo precisa caber
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

### Widgets de módulos no Menu Start

`start_widgets` (`{ x, y, w, h }`, mesmas regras, mas em frações da
composição de 640×480 do Menu Start, não da tela) é a faixa dos widgets 2×1
dos módulos (até 2, lado a lado, divididos por igual). Quando há widget, os
blocos sobem e encolhem para caber acima da faixa e os atalhos vão para logo
abaixo dela; sem widget, o Menu Start fica como sempre. A faixa é
limitada: começa depois de 178 px (os blocos precisam de 72 px), tem pelo
menos 40 px de altura e termina a tempo de os atalhos e o nome deles caberem
acima do rodapé; fora disso, o launcher a encosta no limite. O padrão (maquete
Start-widget):

```toml
start_widgets = { x = 0.153125, y = 0.454166667, w = 0.69375, h = 0.216666667 }
```

`start_area` (`{ x, y, w, h }`, frações da tela) é a caixa onde essa
composição fica centralizada; o padrão é a tela inteira. Veja [Menu Start:
`start_area`](#menu-start-start_area).

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
## Saves e capturas do jogo

As telas Saves e Screenshots (Y › Saves ou Y › Screenshots, na lista de
jogos, quando o jogo tem saves ou capturas) também seguem retângulos do
`theme.toml`, no topo do arquivo, com a mesma regra dos `details_*`: frações
da tela, herança do tema padrão e volta ao padrão quando o valor é inválido.

| chave | conteúdo |
|---|---|
| `saves_list` | lista dos saves (slots e o automático), linhas de 62 px (referência 480 px) com 4 px entre elas, miniatura 64×48 à esquerda; L R vira a página |
| `saves_card` | o save selecionado: a imagem em 4:3 na largura do retângulo, depois Salvo em, Emulador e Tamanho, e a nota sobre o que acontece ao jogar |
| `shots_grid` | grade das capturas: cabem tantas colunas de cerca de 138 px (com 12 px entre elas) quantas a largura deixar, esticadas para preencher; as linhas seguem a altura |

O padrão (maquetes Saves e Screenshots):

```toml
saves_list = { x = 0.01875, y = 0.1, w = 0.4875, h = 0.783333333 }
saves_card = { x = 0.53125, y = 0.1, w = 0.440625, h = 0.783333333 }
shots_grid = { x = 0.040625, y = 0.108333333, w = 0.91875, h = 0.791666667 }
```

As cores vêm das chaves gerais: `accent` na linha e na captura
selecionadas, `surface` e `border` na moldura da imagem, `muted` nas datas e
nas notas, `dim` no tempo relativo, `danger` no botão Apagar da confirmação.
A captura aberta ocupa a tela inteira, com o nome do jogo no alto e os botões
no rodapé; as confirmações (Jogar daqui e Apagar) são janelas centradas.

## Alternador e retomada ao ligar

O Alternador (um toque no FN durante o jogo: o jogo é salvo e fechado e
o launcher abre nos jogos recentes) e a tela de retomada ao ligar
(Configurações › Jogos › Retomar e alternar) seguem retângulos do
`theme.toml`, com a mesma regra dos `details_*`.

| chave | conteúdo |
|---|---|
| `switcher_card` | a imagem do jogo selecionado, com o anel `accent` em volta e o selo "Salvo agora" (ou "Sem retomar") no canto |
| `switcher_side` | os vizinhos, esmaecidos: `w` e `h` são o tamanho deles, `y` o topo e `x` o espaço até a imagem selecionada (o da esquerda e o da direita ficam simétricos; podem sair da tela) |
| `switcher_info` | o nome do jogo (19 px, centralizado), a linha com sistema e quando foi jogado e um ponto por jogo |
| `resume_shot` | a imagem do jogo na tela de retomada |
| `resume_info` | "Voltando para", o nome (22 px), quando foi salvo, a barra da contagem e os botões A e B |

O padrão (maquetes Alternador e Retomar-boot):

```toml
switcher_card = { x = 0.1875, y = 0.108333333, w = 0.625, h = 0.625 }
switcher_side = { x = 0.046875, y = 0.129166667, w = 0.375, h = 0.375 }
switcher_info = { x = 0.0, y = 0.758333333, w = 1.0, h = 0.125 }
resume_shot = { x = 0.25, y = 0.110416667, w = 0.5, h = 0.5 }
resume_info = { x = 0.0, y = 0.6375, w = 1.0, h = 0.258333333 }
```

As cores vêm das chaves gerais: `accent` no anel, no ponto do jogo
selecionado e na barra; `badge` nos outros pontos e no fundo da barra;
`surface` e `border` na caixa de um jogo sem imagem; `muted` nas linhas de
apoio. A imagem é a tela no momento em que o jogo foi salvo; sem ela, a
captura de tela do jogo ou a capa.

## Movendo as ROMs

A tela que aparece no boot de manutenção que move as ROMs para a partição
exFAT (`archr-launcher --roms-migrate-screen`) usa sempre o tema padrão
instalado, nunca o tema escolhido: nesse boot a partição STORAGE, onde ficam
os temas do usuário, é desmontada para encolher, e uma fonte aberta ali a
impediria. Como as Configurações, o layout é fixo. Cores do padrão: `accent`
no ícone, na barra e nos pontos, `border` no trilho, `text` no título,
`muted` no aviso, `text_secondary` na linha do andamento e `dim` no rodapé.

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

- extensões `toml`, `png`, `jpg`, `jpeg`, `webp`, `svg`, `ttf`, `otf`, `ogg`, `wav`,
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

As capturas saem em 640×480. Para outra tela, acrescente
`ARCHR_CAPTURE_SIZE=<largura>x<altura>`; os nomes dos arquivos não mudam,
então use uma pasta por tamanho:

```sh
for tamanho in 640x480 720x720 480x320; do
  mkdir -p /tmp/capturas/$tamanho
  ARCHR_CAPTURE_SIZE=$tamanho \
  ARCHR_SETTINGS_CAPTURE_DIR=/tmp/capturas/$tamanho \
  ARCHR_CAPTURE_VIEWS=praia:lista,praia:vitrine \
  cargo test -p archr-ui --lib capture_views -- --ignored
done
```

Qualquer tamanho vale (`854x480` para 16:9). As outras capturas manuais do
`archr-ui` (Configurações, conquistas, retomada, barra de status) leem a
mesma variável.

O teste `resolutions` confere, em 640×480, 720×720 e 480×320, cada tema com
`theme.toml` em `themes/` e cada visualização dele: lista abaixo do
cabeçalho, nada fora da tela, painel, imagem, texto e pontos sem cobrir a
lista, e os retângulos dos detalhes, saves, Alternador e retomada sem se
sobrepor. Para conferir um tema seu, copie a pasta para `themes/` do
repositório e rode:

```sh
cargo test -p archr-ui --test resolutions
```
