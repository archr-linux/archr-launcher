# Temas do archr-launcher

Um tema é uma pasta com um `theme.toml`. Ele pode trocar cores, fontes, ícones,
imagens de console e também **onde cada coisa fica** nas telas de sistemas e de
jogos. Tudo é opcional: o que o tema não diz vem do tema padrão (Grafite),
chave por chave. O tema escolhido de fábrica (`launcher.theme` sem valor) é o
Ellegance; o Grafite (`default`) continua sendo a base de que todo tema herda.

## Onde ficam

```
/storage/.config/archr-launcher/themes/<nome>/   temas do usuário (têm prioridade)
/usr/share/archr-launcher/themes/<nome>/         temas do sistema
  theme.toml          obrigatório
  fonts/              fontes .ttf citadas no theme.toml (opcional)
  icons/              ícones Lucide .svg que substituem os embutidos (opcional)
  systems/            imagens de console: snes.png, gba.webp... (opcional);
                      app-<id>.png troca a arte de um app de módulo
  sounds/             sons da interface .ogg/.wav (opcional, ver Sons)
```

O tema aparece em Configurações › Aparência › Tema assim que a pasta existe.
Pasta que o tema não traz vem do tema padrão instalado
(`/usr/share/archr-launcher/themes/default/`): sem `fonts/` e sem chaves de
fonte, ele usa as fontes do padrão; sem a imagem de um console, a do padrão;
sem um ícone, o do padrão; sem um som, o do padrão.
Um exemplo completo e pequeno está em `themes/praia/` (aparece como "Exemplo";
o tema Praia completo é publicado à parte, na Loja de temas).
O tema de referência da API 3.1 é `themes/ellegance/` (Ellegance): usa
todas as chaves novas e serve de modelo (veja [Ellegance](#ellegance-o-tema-de-referência-da-31)).

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
| `success`, `warning`, `danger` | estados positivos, de atenção e de falha (`warning` também no selo de atualizações das lojas no Menu Start, com o número na cor `background`) |
| `scrim` | véu atrás de avisos e menus |

O aviso de partição das ROMs com erro (só leitura ou não montada) fica no
rodapé das listas de sistemas e de jogos, no lugar do resumo, com o ícone
`icons/triangle-alert.svg` e a cor `warning`; o layout não muda.

Os avisos (`docs/avisos.md`) usam `surface` e `border`, o texto `text` e
`text_secondary`, e a cor do tipo no ícone e na barra de tempo: `success`
(`icons/check.svg`), `accent` (`icons/info.svg`) ou `warning`
(`icons/triangle-alert.svg`, que também tinge a borda).

### Aparência por cima do tema

Em Aparência, a **1ª bolinha** da Cor de destaque é o `accent` do tema ativo,
com o `accent_text` do próprio tema. As seis cores prontas vêm depois (a que for
igual à do tema some) e a personalizada por último. Quando o usuário escolhe outra
cor, o texto sobre ela fica preto ou branco automaticamente, pelo contraste.

O Fundo começa em **Do tema** (as superfícies do tema); Preto, Azul-noite e uma cor personalizada são
alternativas. **Escolher outro tema** (em Aparência ou em **Usar** na Loja de
temas) traz o tema inteiro, como o autor fez: as escolhas feitas por cima do
tema anterior (cor de destaque, fundo, cores do texto, fonte, peso, tamanho do
texto, animações, visualização, arte dos jogos e o resto da página Aparência)
voltam ao padrão, como em **Restaurar padrão**, e a visualização é a primeira
do tema novo. Idioma, controles, rede e sons não mudam. Depois da troca o
usuário pode personalizar de novo; **Restaurar padrão** volta tudo para o tema
escolhido.

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
font_bold = "fonts/Inter-Bold.ttf"      # opcional: títulos (peso 700)
font_mono = "fonts/JetBrainsMono-Regular.ttf"
font_mono_medium = "fonts/JetBrainsMono-Medium.ttf"
```

Caminhos relativos à pasta do tema, sem `..`. Cada família vem inteira: o tema
que cita `font`, `font_semibold` ou `font_bold` usa as suas (a regular serve de
seminegrito se faltar a outra); o que não cita nenhuma delas herda todas do
tema padrão. `font_bold` desenha os títulos: o nome do sistema no painel e o
nome do jogo nos detalhes. Sem ela, esses títulos usam a seminegrito, como
antes (engrossada quando ela é de peso regular). O mesmo vale para `font_mono` e `font_mono_medium`. Uma fonte citada
que não existe cai nas do sistema (DejaVu/Liberation).

### Tamanho e nitidez do texto

```toml
font_scale = 1.4        # 0,5 a 2: multiplica todo tamanho de texto (com o do usuário)
font_grid = 8           # 0 a 32 px: tamanhos ficam no múltiplo mais próximo (0 = livre)
font_smoothing = false  # sem suavização: letras de borda dura, para fontes pixel
font_min = 12           # 6 a 32 px: nenhum texto fica menor; o que não cabe quebra linha
                        # (rótulos dos blocos do Menu Start podem ficar 2 px abaixo, nunca abaixo de 8)
```

```toml
font_hinting = "light"  # "mono" (padrão), "light" ou "none"
```

`font_hinting` diz como as letras se encaixam na grade de pixels. `"mono"`
(o padrão, aprovado no R36S) prende as hastes nos dois eixos: o texto
pequeno fica mais nítido, um pouco mais largo e espaçado que o desenho da
fonte. `"light"` prende só a altura: larguras e espaçamento da própria fonte,
como num navegador, com hastes um pouco mais macias. `"none"` não prende nada.

`font_grid` serve a fontes desenhadas numa grade (Press Start 2P fica nítida
em múltiplos de 8 px). Vale também nas Configurações. Padrões: `1.0`, `0`, `true` e `8`.

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
Os testes de layout também desenham cada tema em 1024×768 (4:3), 1024×600 e
1280×720 (as telas dos portáteis RK3562, RG43V PRO e RG52 MINI).

O desenho de 640×480 é ampliado ou reduzido até caber inteiro: a escala é a
menor entre largura/640 e altura/480, e o texto fica do mesmo tamanho
físico em telas da mesma densidade. A sobra vai para o lado mais longo, em
px de referência:

| tela | escala | tela em px de referência | formato |
|---|---|---|---|
| 640×480 | 1 | 640×480 | `classic` |
| 720×720 | 1,125 | 640×640 | `square` |
| 480×320 | 0,667 | 720×480 | `wide` |
| 1024×768 | 1,6 | 640×480 | `classic` |
| 1024×600 | 1,25 | 819×480 | `wide` |
| 1280×720 | 1,5 | 853×480 | `wide` |

O que isso faz com cada número do tema:

- **Tamanhos** (fontes, alturas de linha, cabeçalho, rodapé, raios,
  `margin`) não mudam em px de referência. Numa tela quadrada a lista de
  jogos tem linhas do mesmo tamanho e mostra mais delas (pelo menos 13 em
  vez de 10 no Grafite); L/R viram a página pelas linhas que a lista mostra.
- **Retângulos** (`cover`, `details_*`, `switcher_*`, `x y w h` das
  visualizações) são frações da tela e esticam com ela. Uma capa que é
  retrato em 640×480 fica mais alta e estreita em 720×720. A exceção são as
  imagens de jogo do Alternador (`switcher_card`, `switcher_side`) e da
  retomada (`resume_shot`): a imagem fica sempre 4:3, centrada no retângulo
  (estreitada ou encurtada), para não esticar numa tela 16:9.
- A geometria embutida da `lista` (painel do sistema com 250 px de largura,
  margens, folgas sob o cabeçalho) é calculada em px de referência para a
  tela de verdade; ela não estica.

### Tabelas `[aspect.<formato>]`

Para mudar o layout de um formato, use uma tabela `[aspect.<formato>]`. O
formato sai da razão largura/altura da tela:

| formato | razão | telas |
|---|---|---|
| `square` | menor que 1,2 (6:5) | 720×720 |
| `classic` | de 1,2 a 1,45 (29:20), inclusive | 640×480, 800×600, 1024×768 |
| `wide` | maior que 1,45 | 480×320 (3:2), 1024×600, 854×480 e 1280×720 (16:9) |

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
`font_semibold`, `font_bold`, `font_mono`, `font_mono_medium`), o `name`, os sons, os
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

A fileira de atalhos tem até 8 círculos de 46 px com 12 px entre eles, sempre
centrada; quais e em que ordem é escolha do usuário (Configurações › Aparência
› Atalhos do Start), não do tema. O atalho Atualização ganha um ponto
`accent` quando há atualização do sistema; Loja de temas e Loja de módulos
ganham um selo `warning` com quantos temas ou módulos instalados têm versão
nova.

Uma caixa com menos de 480 px de referência de altura aperta a composição.
`start_widgets` (ver [Widgets de módulos no Menu
Start](#widgets-de-módulos-no-menu-start)) é medido dentro dessa
composição, não na tela.

### Configurações

As Configurações têm 640 px de referência de largura, centradas numa tela
mais larga, e ganham linhas numa tela mais alta. O layout é o de sempre até o
tema pedir outro com a tabela [`[settings]`](#configurações-settings).

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

### Sombra, vidro, foco, movimento e gradiente

Chaves do topo desde a 3.1. Todas são opcionais: o tema que não cita nenhuma
desenha exatamente como antes. Diferente das chaves acima, estas só valem nas
Configurações quando o tema as reformula com
[`[settings]`](#configurações-settings). Valor inválido ou de tipo errado gera aviso no
log e fica o padrão; subchave desconhecida também só gera aviso.

| chave | valores | padrão | efeito |
|---|---|---|---|
| `shadow_color` | cor | preta | cor da sombra `"soft"`; a transparência da cor multiplica a da sombra (`"#00000080"` é metade) |
| `shadow_blur` | 0 a 0,1 | ~0,033 (16 px) | quanto a sombra `"soft"` passa da caixa (tamanho, fração de 480); `0` não espalha |
| `shadow_offset` | -0,05 a 0,05 | 8 px | quanto a sombra `"soft"` desce (negativo sobe) |
| `glass` | `{ fill, border, highlight, blur }` | nenhum | o material de vidro (abaixo) |
| `focus` | `"pill"`, `"lift"` | `"pill"` | `"lift"`: o item em foco cresce, ganha a sombra e um contorno, em vez da faixa colorida. Vale para os blocos (`mark = "tile"`), as prateleiras, a biblioteca, a grade de jogos e os blocos do Menu Start; o item que ganha o foco sobe no movimento do tema (a mola, com `motion = "spring"`), o primeiro de cada tela já aparece em cima |
| `focus_scale` | 1 a 1,15 | `1.06` | quanto o item em foco cresce com `"lift"` |
| `focus_ring` | `{ color, width }` | cor `accent`, 2 px | contorno do foco `"lift"`; `width` de 0 a 8 px de referência (`0` sem contorno) |
| `motion` | `"tween"`, `"spring"` | `"tween"` | `"spring"`: tudo que anda segue uma mola criticamente amortecida, que assenta em 0,35 s sem passar do ponto e mantém a velocidade quando o destino muda no meio. Só com Animações Ligadas; Reduzidas e Desligadas seguem como sempre |
| `gradient` | `{ from, to, angle }` | nenhum | fundo das telas de sistemas, jogos e detalhes em degradê, sob a imagem de fundo (se houver) |
| `title_tracking` | -0,1 a 0,1 | `0` | espaço entre as letras dos títulos (texto de 20 px de referência para cima), em em: negativo aperta, como pede o texto grande (o Ellegance usa `-0.022`, perto do -0,02 em dos títulos grandes do iOS). O corpo do texto fica com o espaçamento da fonte |
| `caps_tracking` | 0 a 0,3 | `0` | espaço entre as letras das versaletes sobre um título (o rótulo acima do destaque das prateleiras, "HÁ 2 DIAS" no alternador, "VOLTANDO PARA" na retomada, as seções do menu do jogo), em em. O Ellegance usa `0.12` |

```toml
shadow_color = "#0A204080"
shadow_blur = 0.05
shadow_offset = 0.02
glass = { fill = "#FFFFFF26", border = "#FFFFFF55", highlight = "#FFFFFF80", blur = 0.03 }
focus = "lift"
focus_scale = 1.1
focus_ring = { color = "#FFFFFF", width = 2 }
motion = "spring"
title_tracking = -0.02
gradient = { from = "#1B2A4A", to = "#0A0F1C", angle = 90 }
```

**Espaço entre letras**: o SDL_ttf não tem esse ajuste, então um título com
`title_tracking` é desenhado letra a letra, cada uma onde a linha inteira a
poria (com o kerning da fonte), deslocada pela sua parte do ajuste, e o
resultado vira uma textura só, guardada como qualquer texto: nada é refeito a
cada quadro. As medidas (encolher, quebrar, reticências) contam o ajuste,
então o título sempre cabe onde foi medido. Fora das Configurações reformuladas
por `[settings]`, as Configurações ficam sem ajuste.

**Gradiente**: `from` e `to` são cores (aceitam transparência), `angle` em
graus: `0` vai da esquerda para a direita, `90` de cima para baixo (o padrão é
`0`). O mesmo `{ from, to, angle }` vale no `panel` de uma visualização (veja
[Visualizações](#visualizações)).

**Vidro**: um painel de vidro tem o preenchimento translúcido `fill`, uma borda
de 1 px `border`, um brilho de 1 px `highlight` no topo e, atrás dele, o fundo
desfocado (`blur`, tamanho em fração de 480; `0` desliga). O que faltar vem do
vidro herdado ou do padrão (branco a 12 %, borda a 20 %, brilho a 25 %, sem
desfoque). Um painel escolhe o vidro com `style = "glass"`; sem `glass` no
tema, ele usa esse vidro padrão.

O desfoque nunca é refeito a cada quadro: a imagem de fundo (a do tema ou a do
sistema) é reduzida em passos de metade, uma vez por imagem, e guardada; o
vidro só desenha o pedaço que fica atrás dele, ampliado. Sem imagem de fundo
não há o que desfocar, e o vidro desenha só o preenchimento.

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

A visualização com que o tema abre é a dele: `default_view = "<id>"` no `theme.toml` (uma visualização que existe, própria ou herdada); sem ela, a primeira das visualizações que o tema declara (pela ordem dos ids); sem nenhuma, a do tema base (`lista`). Ao escolher o tema em Aparência, a Visualização passa a ser essa; o usuário ainda pode trocar depois.

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
| `list` | a lista | `x y w h`, `row`, `selected` (altura da linha escolhida), `gap`, `font`, `selected_font`, `radius`, `mark`, `scroll`, `cursor`, `opacity` |
| `panel` | cartão em relevo (painel do sistema, moldura da capa) | `x y w h`, `radius`, `style` (`plain` ou `glass`, ver [vidro](#sombra-vidro-foco-movimento-e-gradiente)), `gradient` (`{ from, to, angle }`, ou `false`), `opacity` |
| `art` | imagem do console ou capa do jogo | `x y w h`, `fit`, `radius`, `opacity` |
| `backdrop` | a capa ao fundo da tela de jogos | `opacity`, `solid` (fração da largura em fundo puro), `edge` (opacidade do fundo na borda direita) |
| `info` | nome do sistema e detalhes | `x y w h`, `title`, `meta` (tamanhos), `align` (`left`/`center`/`right`), `valign` (`top`/`bottom`), `details` (`stacked`: fabricante, nome, contagem; `inline`: nome e "213 jogos · Nintendo · 1990"), `opacity` |
| `meta` | ano, jogadores e "jogado há" sob a capa | `x y w h`, `size`, `opacity` |
| `dots` | pontos de posição entre sistemas | `x y w h` |
| `strip` | faixa de sistemas acima de uma grade de jogos (L1/R1 trocam de sistema; a linha Módulos entra depois de Tools, e na grade dela cada módulo é um cartão com o ícone sobre uma cor fixa do módulo) | `x y w h` |
| `decor` | imagens soltas (ver [Decorações](#decorações)) | lista de `{ image, x, y, scale, frames, fps, layer }` |

Valores de `list`:

- `mark`: `"pill"` (faixa colorida), `"bar"` (barrinha ao lado do texto) ou
  `"tile"` (sistemas em blocos lado a lado, o escolhido grande no centro, com a
  imagem do console; só na tela de sistemas). Num carrossel de blocos, ← e →
  (e os ombros L1/R1, que nessas telas fazem o mesmo) andam um sistema;
  ↑ e ↓ também andam um, para nenhum botão saltar ao fim. Numa lista vertical,
  ↑/↓ andam uma linha e ←/→ uma página, do tamanho que a lista mostra
  (`h` dividido por `row + gap`).
- `mark = "grid"` (só na tela de jogos): os jogos em grade, a capa com o nome
  embaixo. `columns` (1 a 8, padrão 4) dá as colunas; `row` é a altura da
  capa, `gap` o espaço entre capas, `font` o tamanho do nome e `radius` os
  cantos. Na grade ← e → andam um jogo, ↑ e ↓ uma linha, e L1/R1 trocam de
  sistema (na ordem da lista de sistemas), sem empilhar telas: B volta de
  onde a grade foi aberta.
- `scroll`: `"edge"` (rola ao chegar na borda) ou `"center"` (mantém a escolha no meio).
- `cursor`: texto curto (até 4 caracteres, na fonte da lista) desenhado antes
  do item escolhido, piscando meio segundo aceso e meio apagado, por exemplo
  `cursor = "▶"` (a fonte precisa ter o caractere). Com Animações desligadas
  fica aceso. Vazio (o padrão) não desenha nada. Não vale para blocos (`tile`).

`opacity` (0 a 1, padrão 1) desenha o elemento inteiro mais transparente: a
lista com as linhas e a faixa, o painel com a sombra, a imagem, o texto. Um
elemento com opacidade abaixo de 1 é desenhado à parte e posto por cima; use
com moderação nas telas que rolam.

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

## Prateleiras

Uma visualização pode abrir nas prateleiras em vez da lista de sistemas: abas
no topo (Início, Biblioteca, Favoritos), o item em foco em destaque e, abaixo,
fileiras de itens. Liga com `home`:

```toml
[view.vidro]
name = { en = "Shelves", pt_BR = "Prateleiras" }
home = true          # as prateleiras com os valores padrão
```

Ou, para mudar alguma coisa, com tabelas (uma tabela já liga as prateleiras;
`home = true` e `[view.vidro.home.*]` não podem estar no mesmo arquivo, porque
o TOML não deixa `home` ser ao mesmo tempo valor e tabela):

```toml
[view.vidro.home.tabs]               # cápsula de abas no topo
items = ["home", "library", "favorites"]
rect = { x = 0.306, y = 0.029, w = 0.388, h = 0.071 }
style = "glass"                      # "glass" (o vidro do tema) ou "plain"

[view.vidro.home.hero]               # destaque do item em foco (só na Início)
rect = { x = 0.05, y = 0.154, w = 0.469, h = 0.313 }   # rótulo, título, detalhes e botões
art = { x = 0.55, y = 0.146, w = 0.409, h = 0.313, fit = "contain" }
backdrop = { blur = 0.108, opacity = 0.6 }   # a imagem do item, desfocada, atrás de tudo
actions = ["play", "favorite", "more"]
title = 0.071                        # tamanho do título (34 px)

[view.vidro.home.shelves]
order = ["continue", "systems", "apps"]
y = 0.5125                           # topo da primeira prateleira
gap = 0.033                          # entre prateleiras
title = 0.027                        # tamanho do título de cada prateleira
[view.vidro.home.shelves.continue]
item = { w = 0.2625, h = 0.196, radius = 0.033 }
[view.vidro.home.shelves.systems]
item = { w = 0.144, h = 0.121, radius = 0.029 }

[view.vidro.home.library]            # aba Biblioteca: grade de sistemas
rect = { x = 0.05, y = 0.233, w = 0.9, h = 0.733 }
columns = 3
item = 0.275                         # altura de cada cartão
gap = 0.029
radius = 0.042

[view.vidro.home.heading]            # título grande da Biblioteca e dos Favoritos
x = 0.05
y = 0.117
w = 0.9
h = 0.092
size = 0.0625
```

Os valores acima são os padrões (o layout do tema Ellegance em 640×480); o
tema só escreve o que muda. Como em toda visualização, `rect`, `art`,
`library.rect` e `heading` são frações da tela; `item.w` e `item.h` também
(largura e altura), e `radius`, `title`, `gap` e `size` são tamanhos (frações
de 480). Num formato de tela diferente os cartões esticam com a tela: ajuste-os
em `[aspect.square]` ou `[aspect.wide]` se quiser outra proporção.

**Prateleiras** (`order`, na ordem em que aparecem):

| nome | o que mostra |
|---|---|
| `continue` | os jogos recentes, com a imagem de onde pararam (a mesma do Alternador); A continua de onde parou |
| `systems` | os sistemas, com a imagem do console, e por último a linha Módulos (quando há módulo ligado com página; o destaque mostra o ícone de módulo sobre o acento) |
| `favorites` | os jogos favoritos |
| `apps` | os apps de módulos |
| `collections` | Favoritos, Recentes e as outras coleções |

Prateleira vazia some. Não há barra de progresso nos recentes: o launcher não
sabe quanto de um jogo foi jogado, e uma barra sem esse dado mentiria.

**Abas**: `home` (Início: destaque e prateleiras), `library` (Biblioteca: todos
os sistemas em grade; A abre o sistema) e `favorites` (Favoritos: uma
prateleira por sistema, com o nome do jogo embaixo). `items` escolhe quais e
em que ordem.

**Botões**: ↑ e ↓ trocam de prateleira (o destaque segue o item em foco, e a
coluna escolhida fica onde estava); ← e → andam na prateleira; L1 e R1 trocam
de aba. A continua um recente, abre um sistema ou um app, joga um favorito; X
marca ou desmarca favorito; Y abre o menu do jogo (num recente, com Tirar dos
recentes); Start abre o Menu Start. Na Início, B não faz nada; toda tela aberta
a partir das prateleiras volta para elas com B, e o launcher volta para elas
depois de um jogo e ao fechar o Alternador.

**Botões do destaque**: na aba Início, ↑ a partir da primeira prateleira leva o
foco aos botões do destaque (`actions`: Continuar ou Jogar, Favorito, Mais; um
sistema ou um app só tem Abrir). ← e → andam entre eles, A aperta o botão em
foco (Continuar retoma de onde parou, Favorito marca ou desmarca, com a
estrela em `warning` quando já é favorito, Mais abre o menu do jogo ao lado do
botão), ↓ ou B voltam para a prateleira, e ao fechar o menu o foco volta ao
mesmo botão. O botão em foco ganha o contorno `focus_ring`; a prateleira fica
sem contorno enquanto isso.

**Desenho**: as abas usam o [vidro](#sombra-vidro-foco-movimento-e-gradiente)
do tema (ou o padrão), o título do destaque usa `font_bold`, e o item em foco
segue `focus`: com `"lift"` ele cresce, ganha sombra e o contorno `focus_ring`;
com `"pill"` só ganha o contorno. A imagem desfocada do destaque é feita uma
vez por imagem e guardada, como o vidro.

`decor` vale dentro de `[view.<id>.home]` como nas outras telas. Para jogar a
lista de jogos em grade com a faixa de sistemas, use `list.mark = "grid"` e
`strip` em `[view.<id>.games]` (veja [Visualizações](#visualizações)). O tema de
teste `crates/archr-ui/tests/themes/prateleiras/` liga tudo isso.

## Telas livres

As telas de sistemas, de jogos e de detalhes de uma visualização também
podem ser descritas como uma **lista de elementos**, cada um no seu retângulo, em
vez dos lugares fixos da tabela acima (`list`, `panel`, `art`, `info`...).
É o jeito de fazer temas como os do EmulationStation: arte de tela cheia por
sistema, molduras recortadas por cima do conteúdo, texto girado, vários
textos e imagens com dados do sistema e do jogo.

Uma tela é livre quando a sua tabela tem elementos `el`. Sem `el`, nada
muda: os temas de hoje desenham exatamente como antes, e as outras telas
(Menu Start, menus, Configurações) seguem as suas tabelas.

```toml
[view.solo]
name = { en = "Solo", pt_BR = "Solo" }

[[view.solo.systems.el]]
id = "arte"                           # a arte do sistema na tela inteira
kind = "image"
src = ["arts/${system}.jpg", "arts/default.jpg"]
fit = "cover"

[[view.solo.systems.el]]
id = "lista"                          # a lista de sempre, que recebe o d-pad
kind = "list"
rect = { x = 0.6, y = 0.15, w = 0.36, h = 0.68 }
row = 0.075
size = 0.04
mark = "bar"
scroll = "center"
z = 10

[[view.solo.systems.el]]
id = "contagem"
kind = "text"
text = "${system.games}"              # "213 jogos", no idioma do usuário
rect = { x = 0.5, y = 0.868, w = 0.448, h = 0.05 }
align = "right"
size = 0.032
shadow = { color = "#000000A0", x = 0.002, y = 0.002 }
z = 20

[[view.solo.systems.el]]
id = "dicas"
kind = "hints"
rect = { x = 0.565, y = 0.94, w = 0.41, h = 0.05 }
style = "outline"
color = "#FFFFFF"
z = 50
```

Os elementos são desenhados do menor `z` para o maior; com o mesmo `z`, na
ordem do arquivo. A tela não desenha mais nada sozinha: cabeçalho, rodapé,
relógio e dicas só aparecem se forem elementos. As listas `el` de cada tela
não se misturam entre arquivos: a do seu tema substitui a herdada inteira, e
`el = []` volta a tela aos lugares fixos.

### Medidas

| campo | valores | padrão | o que faz |
|---|---|---|---|
| `rect` | `{ x, y, w, h }` | a tela inteira | o retângulo, em frações da tela (`x`, `w` da largura; `y`, `h` da altura). Pode passar da tela: o que sobra não aparece |
| `anchor` | `top-left`, `top`, `top-right`, `left`, `center`, `right`, `bottom-left`, `bottom`, `bottom-right` ou `[x, y]` | `top-left` | o ponto do retângulo que fica em `x`, `y` (`center` põe o meio dele lá, como o `origin 0.5 0.5` do ES) |
| `z` | inteiro, -1000 a 1000 | `0` | a camada |
| `opacity` | 0 a 1 | `1` | transparência do elemento inteiro |
| `rotate` | graus, -360 a 360 | `0` | giro no sentido do relógio |
| `pivot` | `[x, y]`, frações do retângulo | `[0.5, 0.5]` | o centro do giro (`[0, 0.5]` é o meio da borda esquerda) |
| `id` | texto | | o nome, para `base` e para ler o tema |

Todo **tamanho** dentro de um elemento (`size` do texto, `row` da lista,
`radius`, sombra) é uma fração da **altura da tela**: `0.035` é 17 px numa
tela de 480 de altura e 25 px numa de 720. O piso de 10 px de tela vale
para todo texto, como no resto do launcher.

### Tipos de elemento

| `kind` | o que é | campos próprios |
|---|---|---|
| `image` | uma imagem | `src`, `fit`, `align`, `tint`, `radius`, `fallback`, `fallback_text` |
| `text` | um texto, com dados | `text`, `font`, `size`, `color`, `align`, `valign`, `upper`, `shadow`, `wrap`, `line` |
| `rect` | uma caixa de cor | `fill`, `border`, `radius`, `gradient` |
| `frame` | uma imagem do tema em 9-slice sobre o retângulo | `image`, `slice`, `scale`, `tile`, `tint` (cor que multiplica a imagem: um padrão branco vira dessa cor) |
| `panel` | o cartão do launcher (superfície ou vidro) | `radius`, `style` (`plain`, `glass`), `gradient` |
| `list` | a lista da tela, que recebe o d-pad | veja [A lista](#a-lista) |
| `carousel` | os itens da tela numa fila, numa coluna ou numa roda, que recebe o d-pad | veja [Coleções](#coleções-carrossel-e-grade) |
| `grid` | os itens da tela numa grade, que recebe o d-pad | veja [Coleções](#coleções-carrossel-e-grade) |
| `hints` | as dicas de botões da tela | `align`, `style`, `color`, `badge`, `letter_color` |
| `status` | relógio, bateria e rádios | `color` |
| `rating` | a nota do jogo em cinco estrelas | veja [Estrelas](#estrelas-rating) |
| `fields` | uma ficha de rótulo e valor | veja [Ficha](#ficha-fields) |
| `video` | o vídeo do jogo ou do sistema em foco, sem som | veja [Vídeo](#vídeo-video) |
| `actions` | os botões da tela de detalhes (Jogar, Favorito, Coletar de novo); só nela | veja [Tela de detalhes](#tela-de-detalhes) |

Campo desconhecido, valor de tipo errado ou fora da faixa gera aviso no log e
fica de fora (ou é limitado); um elemento sem o essencial (um `kind` que não
existe, uma imagem sem fonte) é ignorado. Nada disso derruba o launcher.

### Imagens

```toml
[[view.lado.games.el]]
kind = "image"
src = ["game.screenshot", "game.cover"]   # a primeira que existir
fallback = "icons/cartucho.png"           # do tema, se o jogo não tem nenhuma
rect = { x = 0.717, y = 0.425, w = 0.5, h = 0.5 }
anchor = "center"
fit = "contain"
radius = 0.02

[[view.lado.games.el]]
kind = "image"
src = "game.logo"
fallback_text = { text = "${game.name}", size = 0.06, upper = true, align = "center" }
rect = { x = 0.717, y = 0.2125, w = 0.433, h = 0.175 }
anchor = "center"

[[view.lado.games.el]]
kind = "image"
src = "icons/calendario.png"
tint = "#F9C440"                          # multiplica as cores da imagem
rect = { x = 0.8, y = 0.7375, w = 0.033, h = 0.033 }
anchor = "left"
when = { has = "game.year" }
```

- `src`: um caminho ou uma lista; vale a primeira que existe. Um caminho é
  relativo à pasta do tema (sem `..`) e pode ter dados: `arts/${system}.jpg`,
  `frames/${aspect}/moldura.png`. `fallback` é mais um caminho do tema, no
  fim da lista. Além de caminhos, `src` aceita as imagens que o launcher
  conhece:

  | fonte | o que é |
  |---|---|
  | `game.cover` | a capa do jogo em foco (`<thumbnail>`, ou `<image>` sem ela) |
  | `game.screenshot` | a captura (`<image>` quando há capa, ou `media/screenshots`) |
  | `game.logo` | o logo do jogo (`<marquee>` ou `media/wheel`); `game.wheel` e `game.marquee` são o mesmo |
  | `system.logo` | o logo do sistema na pasta `logos/` do tema: `logos/<sistema>.svg`, `.png`, `.webp` ou `.jpg`, ou com o nome que ele divide (`megadrive` acha `genesis`) |
  | `system.image` | a imagem do sistema que o launcher mostra: a do usuário, a da pasta `systems/` do tema ou a que vem com o launcher |

  `src = ["system.logo", "system.image"]` com `fallback_text` mostra o logo
  do tema, senão a imagem embutida, senão o nome.
- `fit`: `contain` (inteira, dentro do retângulo; o padrão), `cover` (cobre o
  retângulo, cortada no centro) ou `stretch` (esticada).
- `align`: `left`, `center` ou `right`, para a imagem `contain` mais estreita
  que o retângulo.
- `radius`: cantos arredondados (fração da altura da tela).
- `fallback_text`: quando nenhuma fonte existe, este texto no lugar. Um texto
  simples, ou uma tabela com os campos de `text`.
- As imagens carregam fora da thread de desenho, reduzidas a no máximo 1,5
  vez o tamanho da tela e nunca aumentadas (a tela escala as menores; cache
  em `thumbs/scene`); até chegar, o elemento não aparece. Na tela de
  sistemas, as imagens dos dois sistemas vizinhos carregam logo depois das
  do sistema em foco: uma arte de tela cheia leva de 0,3 a 1 s na primeira
  vez num RK3326, e assim já está pronta quando o d-pad chega nela.
  Formatos: PNG, JPG, WebP e SVG.

### Textos

```toml
[[view.lado.games.el]]
kind = "text"
text = "@description"                     # a frase do launcher: "DESCRIÇÃO"
upper = true
rect = { x = 0.4667, y = 0.7375, w = 0.3, h = 0.04 }
anchor = "left"
font = "bold"
size = 0.029
color = "#FBD679"

[[view.lado.games.el]]
kind = "text"
text = "${game.description}"
rect = { x = 0.4667, y = 0.765, w = 0.5, h = 0.16 }
font = "fonts/FiraSansCondensed-Regular.ttf"
size = 0.0292
line = 1.22
color = "#FFFFFFE6"
wrap = true                               # quebra linha; a última que cabe termina em "…"
```

| campo | valores | padrão |
|---|---|---|
| `text` | texto com `${dado}`, ou `@frase` | obrigatório |
| `font` | `regular`, `semibold`, `bold`, `mono` (as faces do tema) ou um arquivo `.ttf` da pasta do tema | `regular` |
| `size` | 0,01 a 0,3 da altura da tela | `0.035` |
| `color` | cor | o `text` do tema |
| `align` | `left`, `center`, `right` | `left` |
| `valign` | `top`, `middle`, `bottom` | `middle` (`top` com `wrap`) |
| `upper` | `true`/`false` | `false`: em maiúsculas |
| `shadow` | `{ color, x, y }` (deslocamento em frações da altura) | nenhuma |
| `wrap` | `true`/`false` | `false`: uma linha, com "…" se não couber |
| `fit_text` | `ellipsis`, `shrink` | `ellipsis`; `shrink`: a linha que não cabe diminui até 60 % do `size` antes do "…" |
| `line` | 0,8 a 3 | `1.25`: altura da linha, vezes o tamanho da letra |
| `long_text` | `ellipsis`, `scroll` | `ellipsis`: o texto que não cabe termina em "…"; `scroll`: rola com ↑/↓ (só na tela de detalhes, com `wrap = true`) |

Um texto vazio (um jogo sem descrição) não desenha nada. Até 16 arquivos de
fonte por tela; um arquivo que não existe usa a fonte regular do tema.

**Frases**: um `text` que é só `@chave` vira a frase do launcher no idioma do
usuário. `@description` e `@last_played` são próprias das telas livres;
`@genre`, `@players`, `@released`, `@developer`, `@publisher` e `@rating` são
as dos Detalhes do jogo; qualquer chave do catálogo (`@hint.play`) também
vale.

### Dados

| dado | o que é |
|---|---|
| `${system}` | o id do sistema em foco (`snes`; uma coleção sem o `@`: `favorites`; um app, o id do módulo: `itchio`) |
| `${system.name}` | o nome dele ("Super Nintendo") |
| `${system.maker}`, `${system.year}` | fabricante e ano, do catálogo de sistemas |
| `${system.count}`, `${system.games}` | quantos jogos: "213" e "213 jogos" (no idioma) |
| `${system.favorites}` | quantos favoritos |
| `${game.name}`, `${game.description}`, `${game.genre}`, `${game.developer}`, `${game.publisher}` | do jogo em foco (tela de jogos) ou aberto (tela de detalhes) |
| `${game.year}`, `${game.players}` | ano e jogadores ("1 a 2 jogadores") |
| `${game.rating}` | a nota de 0 a 5, com uma casa ("4,5") |
| `${game.last_played}` | quando foi jogado ("há 2 dias") |
| `${list.index}`, `${list.total}` | a posição do item em foco, a partir de 1, e quantos há |
| `${item}` | o id do item em foco: o sistema, ou o nome do arquivo do jogo |
| `${aspect}` | o formato da tela: `classic`, `square` ou `wide` |

Na tela de sistemas não há jogo em foco: os dados `game.*` ficam vazios.
Um nome que não existe fica escrito como está, com aviso no log. As imagens
(`game.cover`, `game.screenshot`, `game.logo`, `system.logo`,
`system.image`) e os vídeos (`game.video`, `system.video`) não são texto:
valem em `src` e em `when.has`.

### Estrelas: `rating`

```toml
[[view.nota.games.el]]
kind = "rating"                            # a nota do jogo, em meias estrelas
rect = { x = 0.7667, y = 0.6625, w = 0.2, h = 0.036 }
anchor = "left"
align = "right"
color = "#F9C440"

[[view.nota.games.el]]
kind = "rating"
star = "icons/estrela.png"                 # a estrela do tema, cheia
star_half = "icons/estrela-meia.png"       # opcional
star_empty = "icons/estrela-vazia.png"     # opcional
size = 0.03
spacing = 0.004
rect = { x = 0.04, y = 0.88, w = 0.3, h = 0.04 }
```

Cinco estrelas para `game.rating` (o único dado com estrelas; `value` pode
dizê-lo), arredondadas a meia estrela. Sem nota, nada aparece.

| campo | valores | padrão |
|---|---|---|
| `star` | imagem do tema: a estrela cheia | a estrela do launcher (cheia, desenhada no tamanho) |
| `star_half`, `star_empty` | imagens do tema: a meia e a vazia | a meia é a cheia cortada ao meio sobre uma vazia; a vazia é a cheia em `empty` |
| `color` | cor que multiplica a estrela cheia | o `text` do tema na estrela do launcher; nenhuma numa imagem do tema |
| `empty` | cor da estrela vazia feita da cheia | `color` |
| `empty_opacity` | 0 a 1 | `0.35`; `1` quando há `empty` ou `star_empty` |
| `size` | o lado de uma estrela, fração da altura da tela | a altura do retângulo |
| `spacing` | o espaço entre duas, fração da altura da tela (até 0,2) | um quinto da estrela |
| `align` | `left`, `center`, `right` | `left` |

### Ficha: `fields`

```toml
[[view.ficha.games.el]]
kind = "fields"
rows = [
  { label = "@genre",     value = "${game.genre}" },
  { label = "@players",   value = "${game.players}" },
  { label = "@released",  value = "${game.year}" },
  { label = "Estúdio",    value = "${game.developer}" },
  { label = "@rating",    rating = "game.rating" },
]
rect = { x = 0.04, y = 0.556, w = 0.42, h = 0.29 }
row = 0.058
size = 0.029
color = "#FFFFFF"
label_color = "#C8C8D4"
upper_labels = true
divider = "#FFFFFF28"
```

Uma linha por item de `rows`, de cima para baixo: o rótulo à esquerda, o
valor à direita no espaço que sobra (diminui até 60 % e termina em "…" se
não couber). Uma linha sem valor some e as de baixo sobem; a linha com
`rating = "game.rating"` mostra as estrelas no lugar do valor, e some quando
o jogo não tem nota. As linhas que não cabem no retângulo ficam de fora.

| campo | o que é | padrão |
|---|---|---|
| `rows` | `{ label, value }` ou `{ label, rating = "game.rating" }`; os dois aceitam `${dado}` e `@frase` | obrigatório |
| `hide_empty` | `false` mostra também as linhas sem valor | `true` |
| `row` | a altura de uma linha, fração da altura da tela | o retângulo dividido pelas linhas |
| `font`, `size`, `color` | a letra dos valores | `regular`, `0.03`, o `text` do tema |
| `label_font`, `label_size`, `label_color` | a letra dos rótulos | a dos valores, na cor `muted` do tema |
| `upper_labels` | rótulos em maiúsculas | `false` |
| `divider` | a cor de uma linha fina sob cada linha | nenhuma |
| `star`, `star_half`, `star_empty`, `empty`, `empty_opacity`, `spacing`, `star_size` | as estrelas da linha da nota, como em `rating` (`star_size` é o `size` de lá; a cor é `color`) | o tamanho da letra dos valores |

### Variáveis

```toml
accent = "$acento"                        # vale também nas chaves do topo

[vars]
acento = "#F9C440"
acento_escuro = "#32270D"
titulo = "fonts/Teko-SemiBold.ttf"
fundo = 0.4

[[view.var.games.el]]
kind = "rect"
fill = "$acento"
opacity = "$fundo"                        # o valor inteiro: continua número
rect = { x = 0, y = 0.9, w = 1, h = 0.1 }

[[view.var.games.el]]
kind = "text"
text = "${system.name}"
font = "$titulo"
color = "$acento_escuro"
rect = { x = 0.02, y = 0.9, w = 0.96, h = 0.1 }
z = 1
```

Um valor que é só `"$nome"` vira a variável inteira, com o tipo dela (cor,
número, caminho). `${nome}` dentro de um texto ou caminho vira o texto da
variável. Os nomes dos dados (`system`, `game.*`, `list.*`, `item`,
`aspect`) nunca são variáveis. As variáveis valem em todo o `theme.toml`,
inclusive nas chaves do topo, e uma tabela `[aspect.square.vars]` troca
variáveis só nesse formato de tela. Variável que não existe gera aviso e o
valor fica como está.

### Condições: `when`

```toml
[[view.cond.systems.el]]
kind = "text"
text = "16-BIT"
rect = { x = 0.04, y = 0.06, w = 0.2, h = 0.06 }
when = { system = ["megadrive", "snes"] }  # só nesses sistemas

[[view.cond.games.el]]
kind = "text"
text = "${list.index} / ${list.total}"
rect = { x = 0.025, y = 0.93, w = 0.3, h = 0.05 }
when = { aspect = "square" }               # só numa tela quadrada

[[view.cond.games.el]]
kind = "text"
text = "${game.developer}"
rect = { x = 0.68, y = 0.86, w = 0.28, h = 0.05 }
when = { has = "game.developer" }          # só quando o jogo tem o dado
```

Uma tabela simples, sem expressões. `system`, `aspect` e `has` aceitam um
nome ou uma lista; com mais de uma chave, todas precisam valer. O elemento
que não vale não é desenhado nem pede imagem. Prefira `when.aspect` a uma
tabela `[aspect.<formato>]`: lá, um `el` substitui a lista inteira da tela.

### Reaproveitar: `base`

```toml
[[view.base.systems.el]]
id = "dicas"
kind = "hints"
rect = { x = 0.565, y = 0.94, w = 0.41, h = 0.05 }
style = "outline"

[[view.base.games.el]]
id = "dicas"
base = "systems.dicas"                     # o elemento da tela de sistemas...
rect = { x = 0.02, y = 0.94, w = 0.6, h = 0.05 }
align = "left"                             # ...com estes campos trocados
```

`base = "<tela>.<id>"` (ou só `"<id>"`, na mesma tela) copia um elemento
da mesma visualização; os campos escritos substituem os dele.

### Herdar uma visualização: `base` e `remove`

```toml
[view.solo]
name = { en = "Solo", pt_BR = "Solo" }

[[view.solo.systems.el]]
id = "faixa"
kind = "rect"
rect = { x = 0.56, y = 0, w = 0.44, h = 1 }
fill = "#000000A0"

[[view.solo.systems.el]]
id = "lista"
kind = "list"
rect = { x = 0.6, y = 0.15, w = 0.36, h = 0.68 }
align = "right"
z = 10

[view.solo-esquerda]
name = { en = "Solo, left", pt_BR = "Solo à esquerda" }
base = "solo"                              # tudo o que a Solo tem...

[view.solo-esquerda.systems]
remove = ["faixa"]                         # ...menos a faixa

[[view.solo-esquerda.systems.el]]
id = "lista"                               # o mesmo id: troca só estes campos
rect = { x = 0.04, y = 0.15, w = 0.36, h = 0.68 }
align = "left"

[[view.solo-esquerda.systems.el]]
id = "rodape"                              # id novo: um elemento a mais
kind = "text"
text = "${system.name}"
rect = { x = 0.04, y = 0.86, w = 0.5, h = 0.06 }
```

`base = "<outra visualização>"` no topo de `[view.<id>]` copia as duas telas
da outra visualização. Um elemento com o mesmo `id` de um herdado troca só os
campos que escreve; um `id` novo entra a mais; `remove = ["id", ...]` na
tabela da tela (`[view.<id>.systems]` ou `.games`) tira elementos herdados.
As outras chaves da tela (como `list`) se juntam às herdadas, e a
visualização herdada pode ela mesma herdar de outra (até 4 níveis). O nome
não é herdado: dê um `name` à nova. `base` que não existe, ou que aponta
para a própria visualização, gera aviso e a visualização fica só com o que
ela mesma escreve.

### Opções do tema: `[options]`

```toml
accent = "$acento"                         # a opção muda a cor do tema inteiro

[vars]
acento = "#F9C440"
arte = "arts"

[options.cor]
name = { en = "Theme tone", pt_BR = "Tom do tema" }
default = "amarelo"
choices.amarelo = { name = { en = "Yellow", pt_BR = "Amarelo" } }
choices.roxo = { name = { en = "Purple", pt_BR = "Roxo" }, vars = { acento = "#A56DE2" } }

[options.fundo]
name = { en = "Art", pt_BR = "Arte" }
default = "claro"
choices.claro = { name = { en = "Light", pt_BR = "Claro" } }
choices.escuro = { name = { en = "Dark", pt_BR = "Escuro" }, vars = { arte = "arts2" } }

[[view.opcoes.systems.el]]
kind = "image"
src = ["${arte}/${system}.png", "${arte}/default.png"]
rect = { x = 0, y = 0, w = 1, h = 1 }
fit = "cover"
```

Cada `[options.<id>]` vira uma linha em Configurações › Aparência, logo
abaixo de Visualização, com o `name` traduzido como os outros textos do
tema; ← e → passam pelas escolhas (`choices.<id>`, cada uma com seu `name`).
Com mais de 3 opções, Aparência mostra uma linha "Opções do tema" que abre
uma página com todas. A escolha aparece na pré-visualização na hora e vale
para o launcher inteiro ao sair de Aparência, sem reiniciar.

As `vars` da escolha valem por cima do `[vars]` do tema, em qualquer lugar
onde a variável é usada: nas telas livres e nas chaves do topo (`accent`,
`background`...). Uma escolha sem `vars` deixa o tema como está. `default`
é a escolha de quem nunca escolheu; sem ele, ou se ele não for uma das
escolhas, vale a primeira. Opções e escolhas aparecem na ordem alfabética
do id (como as visualizações); ids só com letras, números, `_` e `-`. Um
tema tem até 12 opções, e cada opção de 2 a 24 escolhas.

A escolha fica guardada por tema no `system.cfg`, em
`launcher.theme_option.<pasta do tema>.<opção>=<escolha>`, e volta quando o
tema é escolhido de novo. "Restaurar padrão" volta as opções ao `default`.
Uma opção pode trocar também as fontes (`font = "$fonte"`), que o launcher
carrega de novo ao sair de Aparência.

### Vários arquivos: `include`

```toml
include = ["partes/cores.toml", "partes/roda.toml"]
```

`include` fica no topo do `theme.toml` (antes de qualquer `[tabela]`). Cada
arquivo da lista é lido como se as chaves dele estivessem escritas no
`theme.toml`, que vence quando os dois dizem a mesma chave; entre dois
arquivos incluídos, vence o último da lista. Tabelas se juntam chave a chave;
listas (como os `[[...el]]` de uma tela) são trocadas inteiras. Um arquivo
incluído pode ter o seu `include` (até 4 níveis); um arquivo que inclui a si
mesmo, direta ou indiretamente, gera aviso e fica de fora. Os caminhos são
relativos à pasta do tema, sempre dentro dela: `..`, caminho absoluto ou
arquivo que não existe geram aviso. Imagens e fontes citadas num arquivo
incluído também são procuradas a partir da pasta do tema. Um arquivo
incluído, por exemplo `partes/cores.toml`:

```toml
[vars]
acento = "#F9C440"
acento_escuro = "#32270D"
```

### A lista

O elemento `list` é a lista de hoje (`list` da tabela acima) posta onde a
cena diz: o d-pad, L1/R1, o cursor e a rolagem funcionam do mesmo jeito.
Cada tela tem um só elemento que recebe o d-pad (`list`, `carousel` ou
`grid`); um segundo gera aviso e fica de fora. Sem nenhum, a navegação
continua numa lista que não aparece.

```toml
[[view.lista.games.el]]
kind = "list"
rect = { x = 0, y = 0.15, w = 0.433, h = 0.775 }
row = 0.0775
size = 0.0375
font = "fonts/FiraSansCondensed-Medium.ttf"
align = "right"
color = "#FFFFFFCC"
selected = { fill = "#F9C440", color = "#32270D", frame = { image = "frames/selecao.png", slice = 8 } }
loop = true
```

| campo | o que é |
|---|---|
| `row`, `gap`, `radius` | altura da linha, espaço entre linhas e cantos (frações da altura da tela) |
| `selected` | a altura da linha escolhida, ou `{ height, fill, color }`: altura, a cor da faixa e a do texto escolhido |
| `size`, `selected_size` | tamanho da letra das linhas e da escolhida (a escolhida, sem `selected_size`, fica 1 px maior) |
| `selected.frame` | `{ image, slice, scale, tile }`: uma imagem do tema em 9-slice por cima da faixa da linha escolhida (com `fill = "#00000000"`, só a imagem) |
| `color` | a cor das outras linhas |
| `font` | a letra dos nomes: `regular`, `semibold`, `bold`, `mono` ou um arquivo `.ttf` do tema |
| `align` | `left`, `center` ou `right`: onde o nome fica na linha |
| `loop` | `true`: do último item, um passo adiante volta ao primeiro, e vice-versa |
| `mark`, `scroll`, `cursor`, `axis`, `columns` | como na tabela `list` acima (`tile` só em sistemas, `grid` só em jogos) |

Com `row` e sem `selected`, a linha escolhida tem a mesma altura das outras.
Sem `font`, a letra da lista é a do tema (`font`, `font_semibold`).

### Coleções: carrossel e grade

`carousel` e `grid` mostram os **itens da tela**: os sistemas na tela de
sistemas, os jogos (e as pastas) na tela de jogos. Cada item é desenhado
pelo modelo `item`; o item em foco fica como `selected` diz, e os outros na
`opacity` do elemento. Um deles recebe o d-pad no lugar da lista.

```toml
[[view.solo.systems.el]]
id = "logos"                               # coluna de logos à direita, estilo Solo
kind = "carousel"
of = "systems"
axis = "vertical"
visible = 3
item = { image = "system.logo", text = "${item.name}", w = 0.2, h = 0.1 }
rect = { x = 0.5, y = 0.05, w = 0.46, h = 0.8 }
spacing = 0.26                             # entre os meios de dois logos, fração da altura
align = "right"
opacity = 0.5                              # os itens fora de foco
selected = { scale = 2, opacity = 1 }
loop = true
z = 10
```

```toml
[[view.albedo.systems.el]]
id = "roda"                                # a roda do Albedo
kind = "carousel"
of = "systems"
axis = "vertical"
visible = 5
wheel = { angle = 5, pivot = [-10, 0.5] }  # cada posição gira 5° em torno de um ponto à esquerda
item = { image = "system.logo", text = "${item.name}", color = "#FBD679", w = 0.22, h = 0.11 }
rect = { x = 0, y = 0, w = 0.3333, h = 1 }
opacity = 0.5
selected = { scale = 1.375, opacity = 1 }
loop = true
```

```toml
[[view.fila.games.el]]
id = "fila"                                # uma fila de capas com reflexo
kind = "carousel"
of = "games"
axis = "horizontal"
visible = 5
item = { image = ["game.cover", "game.screenshot"], text = "${item.name}", size = 0.03 }
rect = { x = 0, y = 0.2, w = 1, h = 0.42 }
spacing = 0.19                             # fração da largura numa fila
opacity = 0.6
selected = { scale = 1.45, opacity = 1 }
reflection = 0.3
loop = true

[[view.fila.games.el]]
kind = "text"
text = "${game.name}"
rect = { x = 0.1, y = 0.76, w = 0.8, h = 0.08 }
align = "center"
size = 0.055
```

```toml
[[view.grade.games.el]]
id = "capas"                               # grade 3x3 de capas, estilo Solo
kind = "grid"
of = "games"
columns = 3
rows = 3
rect = { x = 0.02, y = 0.17, w = 0.62, h = 0.75 }
spacing = 0.02                             # o espaço entre as células, fração da altura
item = { image = ["game.cover", "game.screenshot"], logo = "game.logo", radius = 0.008 }
opacity = 0.7
selected = { scale = 1.08, opacity = 1, frame = { color = "#FFFFFF", width = 0.005, radius = 0.012 } }
favorite = { corner = "top-right", size = 0.2 }
```

| campo | valores | padrão | o que faz |
|---|---|---|---|
| `of` | `systems`, `games` | os itens da tela | só para ler o tema: a tela de sistemas mostra sistemas e a de jogos, jogos |
| `axis` | `horizontal`, `vertical` | `horizontal` | carrossel: fila ou coluna |
| `visible` | 1 a 15 | `3` | carrossel: quantos itens aparecem inteiros; o de cada ponta some aos poucos. É também a página de L1/R1 |
| `spacing` | 0 a 1 | o retângulo dividido por `visible` | carrossel: a distância entre os meios de dois itens (fração da altura numa coluna, da largura numa fila); grade: o espaço entre células (fração da altura), `0.02` |
| `align` | `left`, `center`, `right` | `center` | carrossel em coluna: o lado do retângulo em que os itens ficam |
| `wheel` | `{ angle, pivot }` | nenhuma | roda: todos os itens no meio, cada posição girada `angle` graus (no sentido do relógio) em torno de `pivot`, em tamanhos de item a partir do canto de cima à esquerda do item (`[-3, 0.5]`) |
| `columns`, `rows` | 1 a 8 | `3`, `3` | grade: colunas e linhas à vista; as linhas rolam uma a uma |
| `opacity` | 0 a 1 | `1` | a dos itens fora de foco |
| `selected` | `{ scale, opacity, tint, color, frame }` | `scale = 1`, `opacity = 1` | o item em foco: tamanho (0,25 a 4 vezes), opacidade, cor que multiplica a imagem, cor do nome do item sem imagem (`color`, senão `tint`) e moldura |
| `selected.frame` | `{ color, width, radius }` ou `{ image, slice, scale, tile }` | nenhuma | uma linha em volta da imagem (por trás dela) ou uma imagem do tema em 9-slice por cima |
| `loop` | `true`/`false` | `false` | do último item, um passo adiante volta ao primeiro |
| `reflection` | 0 a 1 | `0` | carrossel: a metade de baixo da imagem de novo, de cabeça para baixo, sumindo |
| `favorite` | `true`, `false` ou `{ image, corner, size, tint }` | a estrela do launcher no canto de cima à direita, nos jogos | o selo do jogo favorito: imagem do tema, canto (`top-left`, `top-right`, `bottom-left`, `bottom-right`), lado em fração da altura da imagem |

**O modelo `item`**:

| campo | o que é | padrão |
|---|---|---|
| `image` | fonte da imagem, ou lista delas (a primeira que existe), com os dados **do item**: `game.cover`, `game.screenshot`, `game.logo`, `system.logo`, `system.image` ou um caminho do tema (`logos/${item}.png`) | `system.logo` nos sistemas, `game.cover` nos jogos |
| `logo` | outra imagem por cima da primeira, menor e no meio (o logo do jogo sobre a capa) | nenhuma |
| `text` | o texto no lugar da imagem que não existe | `${item.name}` |
| `font`, `size`, `color` | a letra desse texto | como em `text` |
| `w`, `h` | o tamanho do item parado no carrossel, frações da tela | o passo ao longo e o retângulo dividido pelo `scale` do foco através |
| `fit`, `radius` | como numa imagem | `contain`, `0` |
| `fill` | uma caixa de cor por trás de cada item | nenhuma |

Dentro do modelo, `${item}` é o id do item (o sistema, ou o nome do arquivo
do jogo), `${item.name}` o nome dele, e os dados `system.*` e `game.*` são os
do item, não os do item em foco.

O carrossel desliza para o próximo item no tempo das listas do launcher; a
grade rola as linhas e o item em foco cresce. A cor de `selected` (`tint`
na imagem, `color` no nome) chega junto, aos poucos, e sai do item que
perde o foco do mesmo jeito. Com Animações Reduzidas ou Desligadas, tudo
muda de uma vez. O texto de um item é desenhado uma vez,
no tamanho do foco, e só escalado e girado ao deslizar.

As imagens dos itens carregam como as dos outros elementos (fora da
thread de desenho, sem aumentar), as do item em foco primeiro.

### Navegação

Uma tela livre com `carousel`, `grid` ou `list` com `loop` segue a mesma
regra em todos os temas:

| elemento | ↑ / ↓ | ← / → | L1 / R1 |
|---|---|---|---|
| lista ou carrossel em coluna (e roda) | um item | uma página | uma página |
| carrossel em fila | um item | um item | uma página |
| grade | uma linha | um item | uma página |

- Página: `visible` no carrossel, `columns × rows` na grade, as linhas à
  vista na lista. Uma página para no primeiro ou no último item.
- Com `loop = true`, o passo de um item passa do último ao primeiro e do
  primeiro ao último. Na grade, ↑/↓ param na primeira e na última linha.
- A, B, X, Y, Start e Select fazem o mesmo de sempre.
- Uma lista sem `loop` (e a lista com `mark = "grid"`, em que L1/R1 trocam
  de sistema) continua como nas telas de sempre.

### Dicas e status

`hints` desenha as dicas de botões da tela (as mesmas do rodapé), juntas no
lado `align` (`left`, `center`, `right`) do retângulo, centradas na altura
dele. `style`: `filled` (a letra num círculo cheio, o padrão), `outline` (a
letra num anel) ou `text` (só a letra). `color` pinta os textos e o círculo
ou o anel; `badge` troca só a cor do círculo ou do anel; `letter_color`, a da
letra num círculo cheio. A opção Dicas só com texto da Aparência continua
valendo.

`status` desenha a pílula de relógio, bateria e rádios na borda direita do
retângulo, centrada na altura dele. Com `color`, os ícones e o texto vão
direto sobre a tela, nessa cor, sem a pílula.

### Vídeo: `video`

```toml
[[view.lado.games.el]]
kind = "video"
src = "game.video"                        # o vídeo do jogo em foco
fallback = ["game.screenshot", "game.cover"]   # até o vídeo começar, e sem vídeo
rect = { x = 0.717, y = 0.425, w = 0.5, h = 0.5 }
anchor = "center"
fit = "cover"
radius = 0.02
delay = 1                                 # segundos parado no jogo antes de tocar
z = 6

[[view.lado.systems.el]]
kind = "video"
src = "system.video"                      # videos/<sistema>.mp4 da pasta do tema
fallback = "system.image"
rect = { x = 0.72, y = 0.5, w = 0.5, h = 0.8 }
anchor = "center"
fit = "contain"
loop = false                              # toca uma vez e volta à imagem
```

| campo | valores | padrão | o que faz |
|---|---|---|---|
| `src` | um vídeo ou uma lista | | vale o primeiro que existe: `game.video` (o vídeo do scraper do jogo em foco), `system.video` (`videos/<sistema>.mp4` na pasta do tema, ou com o nome que ele divide: `megadrive` acha `genesis.mp4`) ou um caminho `.mp4` do tema, que pode ter dados (`videos/${system}.mp4`) |
| `fallback` | uma imagem ou uma lista | | a imagem no lugar do vídeo, com as fontes de `src` da [imagem](#imagens): antes de ele começar, depois que ele acaba (`loop = false`), sem vídeo e com a opção de vídeo desligada |
| `fit` | `cover`, `contain`, `stretch` | `cover` | como na imagem; em `contain`, as faixas em volta ficam transparentes. Vale também para o `fallback` |
| `radius` | fração da altura da tela | `0` | cantos arredondados, do vídeo e do `fallback` |
| `delay` | segundos, 0 a 10 | `1` | quanto tempo parado no item antes de tocar; o mesmo segundo dos detalhes |
| `loop` | `true`, `false` | `true` | recomeça no fim; com `false`, toca uma vez e volta ao `fallback` |
| `mute` | `true` | `true` | o vídeo é sempre sem som, como nos detalhes; `false` gera aviso e fica mudo |

- Um vídeo por tela, na tela de sistemas, de jogos ou de detalhes; um segundo `video`
  gera aviso e fica de fora.
- Toca com o mesmo tocador da prévia dos detalhes (o ffmpeg num processo à
  parte, uma linha de decodificação, 25 quadros por segundo) e segue a opção
  **Vídeo de prévia** das Configurações: desligada, só aparece o
  `fallback`.
- Para ao trocar de item (o próximo espera o seu `delay`), ao sair da
  tela, ao abrir um menu, as Configurações ou um app por cima, e ao abrir
  um jogo. Fora da tela, nada é decodificado.
- O vídeo é decodificado no formato do retângulo e com no máximo os pixels
  de um 640×480; um retângulo maior é preenchido pela GPU. Assim o custo
  no RK3326 é o mesmo da prévia dos detalhes, em qualquer tamanho de tela.
- `opacity`, `rotate` e `when` valem como nos outros elementos
  (`when = { has = "game.video" }` desenha o elemento só com vídeo).

### Animações: `enter` e `select`

Qualquer elemento pode entrar em movimento: `enter` toca quando a tela
abre (ao abrir a tela de sistemas, de jogos ou de detalhes, ao voltar de um jogo, das
Configurações ou de outra tela), e `select` toca a cada troca do item em
foco, para os elementos que mostram dados dele (imagem e vídeo do jogo ou
do sistema, nome, descrição, nota, ficha).

```toml
[[view.albedo.systems.el]]
id = "capa-sistema"
kind = "image"
src = ["systems/${system}.png"]
rect = { x = 0.7167, y = 0.5, w = 0.5, h = 0.8 }
anchor = "center"
fit = "contain"
enter = { from = "left", distance = 0.035, duration = 0.5 }    # desliza um pouco ao abrir
select = { from = "right", distance = 0.025, duration = 0.2 }  # e a cada sistema

[[view.albedo.systems.el]]
id = "contagem"
kind = "text"
text = "${system.games}"
rect = { x = 0.392, y = 0.075, w = 0.86, h = 0.05 }
rotate = 90
pivot = [0, 0.5]
enter = { duration = 0.4, delay = 0.2, ease = "linear" }       # só aparece, depois dos outros

[[view.albedo.games.el]]
id = "logo-jogo"
kind = "image"
src = "game.logo"
rect = { x = 0.717, y = 0.2125, w = 0.433, h = 0.175 }
anchor = "center"
fit = "contain"
select = { scale = 0.9, duration = 0.2 }                       # cresce um pouco ao trocar de jogo
```

| campo | valores | padrão | o que faz |
|---|---|---|---|
| `from` | `left`, `right`, `top`, `bottom`, `none` | `none` | de que lado o elemento chega |
| `distance` | 0 a 1, fração da tela | `0.04` | de quão longe ele chega (`1` vem de fora da tela) |
| `fade` | `true`, `false` | `true` | chega transparente e aparece |
| `scale` | 0,1 a 4 | `1` | o tamanho com que chega (`0.9` cresce até o seu, `1.2` encolhe) |
| `duration` | segundos, 0,05 a 0,6 | `0.3` no `enter`, `0.18` no `select` | quanto tempo leva |
| `delay` | segundos, 0 a 0,6 | `0` | espera antes de começar, para os elementos chegarem um depois do outro |
| `ease` | `out`, `in-out`, `linear` | `out` | a curva: `out` começa rápido e assenta devagar |

- Nada passa de 0,6 s: `delay` + `duration` maior que isso gera aviso e o
  `delay` é cortado. Um `enter` ou `select` que não move, não aparece nem
  cresce gera aviso e fica de fora.
- O elemento está no seu lugar (`rect`, `opacity`) quando a animação acaba;
  ela só diz de onde ele vem. Com `enter` e `select`, os dois se somam.
- Na entrada da tela, `select` não toca; ele toca a cada troca de item. Com
  o d-pad seguro (ou trocas a menos de 0,15 s uma da outra), o item novo
  aparece de uma vez, sem animação, para a lista não engasgar.
- Qualquer botão termina as animações em curso na hora: nada espera a
  animação para responder.
- **Animações Reduzidas**: só um esmaecer curto (até 0,12 s), sem
  deslizar, crescer nem esperar o `delay`. **Desligadas**: nada se mexe.
- Textos, notas, fichas, coleções, dicas e status que crescem, e a lista
  que desliza, são desenhados numa camada da tela durante a animação;
  imagens, caixas e vídeo só mudam de retângulo. Na lista vale a
  posição do `rect` dela.

### Tela de detalhes

A tela de detalhes do jogo (Y › Detalhes) também pode ser livre, com os
mesmos elementos e dados das outras: `[[view.<id>.details.el]]`. Os dados
`game.*` são os do jogo aberto e `${list.index}` é a posição dele entre os
jogos do sistema. Sem `details`, a visualização usa a tela de detalhes de
sempre (as chaves `details_*` e `[details]`).

```toml
[[view.solo.details.el]]
id = "capa"
kind = "image"
src = ["game.cover", "game.screenshot"]
rect = { x = 0.04, y = 0.17, w = 0.42, h = 0.37 }
fit = "contain"

[[view.solo.details.el]]
id = "ficha"
kind = "fields"
rows = [
  { label = "@genre",       value = "${game.genre}" },
  { label = "@players",     value = "${game.players}" },
  { label = "@last_played", value = "${game.last_played}" },
  { label = "@rating",      rating = "game.rating" },
]
rect = { x = 0.04, y = 0.556, w = 0.42, h = 0.29 }
row = 0.058
size = 0.029

[[view.solo.details.el]]
id = "descricao"
kind = "text"
text = "${game.description}"
rect = { x = 0.52, y = 0.69, w = 0.44, h = 0.14 }
size = 0.029
wrap = true
long_text = "scroll"                       # ↑/↓ rolam o texto que não cabe

[[view.solo.details.el]]
id = "botoes"
kind = "actions"                           # Jogar, Favorito, Coletar de novo
rect = { x = 0.52, y = 0.845, w = 0.44, h = 0.07 }

[[view.solo.details.el]]
id = "dicas"
kind = "hints"
rect = { x = 0.4, y = 0.94, w = 0.575, h = 0.05 }
```

Para a tela de detalhes ser a tela de jogos com outra coisa no lugar da
lista, ela herda os elementos da tela de jogos da mesma visualização:

```toml
[[view.albedo.games.el]]
id = "lista"
kind = "list"
rect = { x = 0, y = 0.15, w = 0.433, h = 0.775 }

[[view.albedo.games.el]]
id = "descricao"
kind = "text"
text = "${game.description}"
rect = { x = 0.4667, y = 0.765, w = 0.5, h = 0.16 }
wrap = true

[[view.albedo.games.el]]
id = "dicas"
kind = "hints"
rect = { x = 0.4, y = 0.925, w = 0.575, h = 0.075 }

[view.albedo.details]
base = "games"                             # todos os elementos da tela de jogos...
remove = ["lista"]                         # ...menos a lista

[[view.albedo.details.el]]
id = "descricao"                           # o mesmo id: troca só este campo
long_text = "scroll"

[[view.albedo.details.el]]
id = "capa"                                # id novo: um elemento a mais
kind = "image"
src = "game.cover"
rect = { x = 0.03, y = 0.175, w = 0.373, h = 0.43 }
```

- `base = "games"` é o único valor; `remove` e os elementos com o mesmo
  `id` funcionam como em [Herdar uma visualização](#herdar-uma-visualização-base-e-remove).
  Um elemento herdado com `base = "<id>"` continua apontando para a tela de
  jogos. A visualização que herda outra (`base` no topo) herda também a
  tela de detalhes dela.
- A tela de detalhes não tem lista, carrossel nem grade (geram aviso e
  ficam de fora). Os botões continuam os de sempre: A faz a ação em foco
  (Jogar, Favorito ou Coletar de novo), L1/R1 trocam o foco, ↑/↓ rolam a
  descrição, Y abre as opções do jogo e B volta. As `hints` mostram esses
  botões, com ↕ só quando há texto para rolar.
- `actions` desenha os três botões no retângulo, o que está em foco aceso,
  na aparência dos botões do tema. Sem ele, a ação em foco aparece só na
  dica do A.
- `long_text = "scroll"`: o texto com `wrap = true` que passa do retângulo
  rola uma linha por toque em ↑/↓; enquanto há mais embaixo, a última linha
  aparece esmaecida. Um texto por tela rola; um segundo, ou `long_text` nas
  telas de sistemas e jogos, gera aviso e termina em "…".
- O `video` da tela de detalhes toca no lugar da prévia de sempre, com o
  mesmo tocador e a mesma opção Vídeo de prévia. As animações `enter`
  tocam ao abrir os detalhes, e as opções do tema (`[options]`, variáveis)
  valem como nas outras telas.

### O que as opções da Aparência fazem numa tela livre

Capa do jogo, Largura e Cantos da capa e Fundo por sistema não mexem numa
tela livre: quem decide as imagens é o tema. Cor de destaque, cores do texto,
Fundo e Dicas só com texto valem para a lista, as dicas e o status quando o
elemento não dá a sua cor.

### O que ainda não existe

As telas livres chegam por partes. Ainda não há: vídeo com som, vídeo
aleatório de um jogo do sistema na tela de sistemas, `include` dentro de uma
escolha de opção e texto que rola sozinho (`long_text` só rola com ↑/↓, na
tela de detalhes). Chaves dessas partes geram
aviso e ficam de fora.

### Testar

O tema de teste `crates/archr-ui/tests/themes/cena/` tem as visualizações
das maquetes aprovadas: Solo e Albedo, e as coleções delas, Carrossel
(coluna de logos e grade 3×3) e Roda (a roda de sistemas e uma fila de
capas com reflexo, no arquivo incluído `parts/roda.toml`). "Solo escuro"
herda a Solo, e duas opções ("Tom do tema" e "Arte") trocam a cor e a
arte de fundo. A Albedo tem animações de entrada e de troca. A Solo e a
Albedo têm também a tela de detalhes (a da Albedo herda a tela de jogos), e
o Solo escuro põe os botões nela; as capturas `*-detalhes-*` mostram a
descrição longa no começo, rolada e no fim. As capturas
saem em 640×480, 720×720 e 1024×768, com um quadro no meio do deslize e
outro no meio das animações, com Animações Ligadas, Reduzidas e
Desligadas:

```sh
ARCHR_SCENE_CAPTURE_DIR=/tmp/cena cargo test -p archr-ui --test scene_render
```

O mesmo teste confere que cada exemplo deste capítulo é lido sem nenhum
aviso.

## Converter um tema do EmulationStation

O `archr-es2theme` converte um tema do EmulationStation no formato do
Batocera/ArkOS (`theme.xml`, `formatVersion 7`, subsets, `customView`) num
tema de telas livres. Ele roda no PC; o launcher nunca lê XML. Temas do
ES-DE (`capabilities.xml`, `variant`, `colorScheme`, `aspectRatio`) são lidos
pelo mesmo caminho, com menos cobertura.

```sh
cargo run --release -p archr-es2theme -- \
  ~/temas-es/es-theme-albedo ~/temas-arch/albedo --lang pt --name "Albedo"
```

`--lang` escolhe os textos do tema original (`en`, `pt`, `es`...); os nomes
das opções saem em inglês e português quando o tema tem os dois. A pasta de
saída fica pronta para copiar para
`/storage/.config/archr-launcher/themes/` ou para publicar na Loja:

| arquivo | o que é |
|---|---|
| `theme.toml` | nome, licença, cores do topo e o `include` das partes |
| `parts/vars.toml` | as variáveis e o que muda em cada formato de tela (`[aspect.<formato>.vars]`) |
| `parts/options.toml` | as opções do tema, uma por subset que muda algo |
| `parts/view-<id>.toml` | uma visualização por view do ES (`detailed`, `grid`, cada `customView`) |
| `assets/`, `_inc/`... | as imagens e fontes que o tema usa, nos mesmos caminhos do original |
| `logos/` | os logos de sistema, para o `system.logo` achar também o nome que o sistema divide |
| `original/` | LICENSE, README e créditos do tema original |
| `CONVERSAO.md` | o relatório: o que virou o quê, o que ficou de fora e os avisos do launcher |

O conversor termina lendo o resultado com o leitor de temas do launcher, em
640×480, 720×720, 1024×768 e 854×480, com cada escolha de cada opção; os
avisos vão para o relatório e para a saída (código 2 quando há algum).

**Como ele lê o tema**: segue os `include` (com `lang`), as variáveis
(`${nome}`), os subsets e as condições (`ifSubset`; `ifHelpPrompts` vale
como verdadeiro, porque o launcher mostra as dicas; `ifArch` como falso),
e junta os elementos pelo nome, como o ES. O tema é lido uma vez por formato
de tela (o subset de proporção, quando há, na escolha mais próxima: 4:3 no
`classic`, 1:1 no `square`, 16:9 no `wide`) e uma vez por escolha de cada
outro subset. O que muda entre as leituras vira variável: por formato em
`[aspect.<formato>.vars]`, por escolha em `[options.<id>]`. Um elemento que
muda com o formato e com uma opção ao mesmo tempo é escrito uma vez por
formato (`when.aspect`).

| ES | tema do launcher |
|---|---|
| `pos`, `size`, `maxSize`, `minSize`, `origin` | `rect`, `anchor`, `fit` (`contain`, `cover`, `stretch`) |
| `zIndex`, `rotation`, `rotationOrigin`, `opacity`, `visible` | `z`, `rotate`, `pivot`, `opacity`; invisível fica de fora |
| `image` (extra), com `${system.theme}` | `image` com `src` em lista, `${system}` no caminho |
| `image` com `tile` | `frame` com `slice = 0` e `tile = true` (o `color` vira `tint`) |
| `text`, `datetime`, `md_*` | `text` com `${game.*}`, `${system.*}` ou `@frase` |
| `textlist` | `list` (cor, letra, seleção, alinhamento) |
| `imagegrid` / `gridtile` | `grid`, ou `carousel` quando é uma linha ou uma coluna (zoom, reflexo, favorito, logo por cima) |
| `carousel` (horizontal, vertical, `vertical_wheel`) | `carousel` de sistemas (`wheel` na roda) |
| `md_image`, `md_marquee`, `md_video`, `md_rating` | `image` (`game.screenshot`...), `video`, `rating` |
| `helpsystem`; `clock` e `batteryIndicator` | `hints`; `status` |
| `storyboard` (x, y, opacity, scale) | `enter` |
| `ninepatch` | `frame` |

A tela de detalhes de cada visualização é a de jogos com a capa, a ficha
e os botões no lugar da lista, e a descrição rolando com ↑/↓.

**O que fica de fora** (e está no relatório): sons e música de fundo, a
tela `menu` do ES, `splash`, vídeo ou imagem aleatória (`{random}`),
arquivos fora do tema (`~/roms/...`, `:/...`), vídeo dentro da grade, os
estados `:selected`/`:videoplaying` do cartão e `animateColor`, o nome
embaixo de cada cartão da grade, `controllerActivity`, contagem de partidas.
A data de lançamento vira só o ano; as dicas de botões usam a letra do
launcher. Um elemento que o ES desenha sem o tema citá-lo não é criado.

**Revise antes de publicar**: o resultado é um ponto de partida. As medidas
são as do ES, mas a letra, a altura da linha da lista e as imagens com
`${system}` (os nomes de sistema do tema original) podem pedir ajuste à mão.
Veja as capturas com o teste de renderização:

```sh
ARCHR_ES2THEME_RENDER=~/temas-arch/albedo \
ARCHR_ES2THEME_SHOTS=/tmp/capturas-es \
cargo test -p archr-es2theme --test render
```

Ele desenha as telas de sistemas, de jogos e de detalhes de cada
visualização em 640×480, 720×720 e 1024×768;
`ARCHR_ES2THEME_CHOICES=opcao=escolha,...` escolhe opções do tema.

**Licença**: o conversor não traz arte nenhuma. Ele copia a arte, as fontes
e os logos do tema original para a pasta de saída e guarda a licença e os
créditos em `original/`; a `license` do `theme.toml` diz a do original. Um
tema com licença não comercial (CC BY-NC, CC BY-NC-SA) não pode ir nas
imagens do Arch R nem na Loja sem autorização por escrito do autor; o
relatório avisa. Logos de sistemas são marcas dos fabricantes.

## Uma tabela por tela

Desde a 3.1 cada tela pode ter uma tabela no topo do `theme.toml`, fora de
`[view]` (vale para todas as visualizações). Tudo é opcional e herda do tema
padrão; chave desconhecida ou valor errado gera aviso no log e fica o padrão.
Um tema sem essas tabelas desenha exatamente como antes.

Além das posições, cada tabela escolhe um estilo de desenho; sem a chave, a
tela é desenhada como sempre:

| tabela | chave do estilo | o que muda |
|---|---|---|
| `[start]` | `style = "control"`, `tiles`, `shortcuts`, `clock` | central de controle sobre a tela anterior desfocada |
| `[menu]` | `style = "context"`, `rect` | cartão ao lado do item em foco, com ações rápidas |
| `[details]` | `style = "hero"` | destaque de tela cheia com a arte desfocada |
| `[achievements]` | `progress = "ring"`, `style = "glass"` | anel de progresso, lista em vidro |
| `[saves]` | `style = "preview"` | prévia grande e vagas em vidro |
| `[shots]` | `sections = true`, `columns` | grade por data |
| `[switcher]` | `cards = "depth"` | cartões em profundidade |
| `[resume]` | `style = "title"`, `progress`, `backdrop` | título grande no meio |
| `[search]` | `style = "field"`, `field`, `results`, `dock` | campo e resultados em vidro, teclado encaixado |
| `[keyboard]` | `sheet = true`, `key` | edição numa folha, teclas do tema |
| `[header]` | `style = "capsule"` | conquistas, saves, capturas e busca com "‹ <jogo>" numa cápsula e o título grande ([Cabeçalho](#cabeçalho-das-telas-do-jogo-header)) |
| `[dialog]` | `buttons = "row"`, `style` | perguntas em cápsulas, destrutivas em vermelho; a de apagar ou carregar um save vira um alerta centralizado |
| `[status]` | `rect` | onde fica a pílula do relógio |
| `[osd]` | `style = "capsule"`, `position` | indicador de volume e brilho em cápsula de vidro ([OSD](#osd-de-volume-e-brilho-osd)) |
| `[settings]` | `layout = "sidebar"`, `icon = "tile"`, `groups = "inset"`, `title = "below"` | Configurações em barra lateral, ícones em quadrados coloridos, linhas em grupos, título numa linha própria ([Configurações](#configurações-settings)) |

O tema de teste `crates/archr-ui/tests/themes/prateleiras` usa todos; o teste
`home_render` desenha cada tela em 640x480, 720x720 e 480x320
(`ARCHR_HOME_CAPTURE_DIR=<pasta> cargo test -p archr-ui --test home_render`
grava os PNGs).

### Ellegance, o tema de referência da 3.1

`themes/ellegance/` é um tema completo que usa toda a API 3.1: estilo
(`font_bold`, sombra, vidro, foco `"lift"`, `motion = "spring"`, gradiente),
prateleiras, uma tabela por tela, Configurações em barra lateral, o OSD em
cápsula e `[aspect.square]`/`[aspect.wide]` para 720x720 e 480x320. Copie a
pasta e mude o que quiser. O começo dele:

```toml
name = "Ellegance"
font_bold = "fonts/Inter-Bold.ttf"
shadow_color = "#000000F0"
shadow_blur = 0.075
shadow_offset = 0.0375
glass = { fill = "#FFFFFF14", border = "#FFFFFF1F", highlight = "#FFFFFF29", blur = 0.05 }
focus = "lift"
motion = "spring"
title_tracking = -0.022
caps_tracking = 0.12
gradient = { from = "#121219", to = "#07070A", angle = 90 }

[hints]
style = "capsule"
lit = true

[start]
style = "control"

[header]
style = "capsule"

[settings]
layout = "sidebar"
icon = "tile"
groups = "inset"
title = "below"

# "lista" é a visualização de quem acabou de instalar: aqui, as prateleiras.
[view.lista.home.shelves]
order = ["continue", "systems", "apps"]

[view.lista.games]
list = { x = 0.05, y = 0.31, w = 0.9, h = 0.57, row = 0.2167, mark = "grid", columns = 4 }
strip = { x = 0.05, y = 0.225, w = 0.9, h = 0.06 }

# Os apps de módulos numa página de loja, em qualquer visualização.
[app]
header = "large"

# Para quem prefere a lista de sempre.
[view.classica]
name = { en = "List", pt_BR = "Lista" }
```

Um tema não escolhe a visualização inicial: ela é a `launcher.view` do
usuário, `lista` numa instalação nova. Por isso o Ellegance põe as
prateleiras na própria `lista` e oferece a lista clássica como `classica`.

O teste `home_render` desenha todas as telas do Ellegance nos três tamanhos
e compara todas as de 640x480 com `tests/golden/ellegance-640x480`, inclusive
as que mostram datas e a barra de tempo de um aviso: o relógio do launcher
fica parado num instante fixo em UTC (`Ui::pin_clock`) e cada quadro espera
todas as imagens pedidas carregarem antes de ser guardado
(`ARCHR_ELLEGANCE_CAPTURE_DIR=<pasta>` grava os PNGs;
`ARCHR_GOLDEN_UPDATE=1` regrava as referências). O teste `shipped_themes`
confere que todo tema de `themes/` carrega sem um aviso no log.

### Posições das telas

`[details]`, `[achievements]`, `[saves]`, `[shots]`, `[switcher]`, `[resume]` e
`[start]` reúnem os retângulos que já existiam soltos no topo. As chaves
antigas continuam valendo; se as duas formas aparecem, vale a da tabela.

| tabela e chave | chave antiga |
|---|---|
| `[details]` `shot`, `info`, `title`, `subtitle`, `text`, `actions` | `details_shot` ... `details_actions` |
| `[achievements]` `progress`, `list` | `achievements_progress`, `achievements_list` |
| `[saves]` `list`, `preview` | `saves_list`, `saves_card` |
| `[shots]` `grid` | `shots_grid` |
| `[switcher]` `card`, `side`, `info` | `switcher_card`, `switcher_side`, `switcher_info` |
| `[resume]` `shot`, `info` | `resume_shot`, `resume_info` |
| `[start]` `area`, `widgets` | `start_area`, `start_widgets` |

```toml
[saves]
list = { x = 0.55, y = 0.12, w = 0.4, h = 0.7 }
preview = { x = 0.05, y = 0.12, w = 0.47, h = 0.5 }
```

### Avisos: `[toast]`

```toml
[toast]
position = "top"      # "top-left", "top" ou "top-right"; sem a chave, o canto que o usuário escolheu
width = 0.75          # largura do cartão (tamanho, fração de 480; padrão 0,575 = 276 px)
icon = "circle"       # "circle", "square" ou "none"
style = "glass"       # "glass" (o vidro do tema) ou "plain"
```

Em `"top"` o aviso fica centralizado e desce do topo em vez de deslizar do
lado, como uma faixa por cima do cabeçalho (a 12 px do topo). Em `"glass"` o
cartão tem a `surface` do tema por baixo do vidro (nada da tela aparece
através do texto) e os cantos de `radius_card`. Vidro sobre uma tela
escurecida (perguntas, a folha do teclado) leva a mesma base.

### Dicas de botões: `[hints]`

```toml
[hints]
style = "capsule"     # "bar" (como sempre) ou "capsule": as dicas numa cápsula de vidro
position = "right"    # "left", "center" ou "right"
lit = true            # a dica do A acesa (selo na cor do texto, rótulo em `text`),
                      # as outras em `text_secondary`; sem ela, todas em `muted`
```

Uma camada sobre a tela (pergunta, menu, teclado, pergunta ou teclado de um
app) mostra só as dicas dela, com o brilho normal, nunca as da tela de baixo
apagadas sob o escurecimento. No teclado em folha (`[keyboard] sheet`) as
dicas ficam dentro da folha, sem cápsula própria por cima dela.

Vale para o rodapé das telas de sistemas, jogos, detalhes, das prateleiras,
das conquistas, dos saves, das capturas e do alternador (e das perguntas
dessas telas). Com a cápsula, use `divider = false` na visualização para tirar
a linha acima do rodapé das telas de sistemas e jogos; nas outras ela some
sozinha quando `[hints]` não é o padrão.

### Relógio e bateria: `[status]`

```toml
[status]
style = "glass"       # a pílula no vidro do tema
battery = true        # false: sem a bateria
radios = true         # false: sem os ícones de Wi-Fi e Bluetooth
clock = "text"        # "mono" (padrão, discreto) ou "text": a hora na seminegrito, na cor do texto
rect = { x = 0.5, y = 0.02, w = 0.46, h = 0.08 }  # opcional: onde fica a pílula
```

Com `rect`, a pílula encosta à direita do retângulo e fica centralizada na
altura dele; sem a chave, fica à direita do cabeçalho, como sempre.

### Cabeçalho das telas do jogo: `[header]`

```toml
[header]
style = "capsule"     # "path" (padrão): o caminho em texto; "capsule": "‹ <jogo>" numa cápsula
```

Vale para as telas que se abrem de um jogo: Conquistas, Saves, Capturas e
Busca. Com `"capsule"` o caminho em texto ("Super Nintendo / Kirby Super
Star / Saves") dá lugar a uma cápsula de vidro "‹ <jogo>" no alto à
esquerda (a mesma dos detalhes em `[details] style = "hero"`), com o relógio
à direita, e ao título da tela grande (28 px, `font_bold`, com
`title_tracking`) numa linha própria embaixo dela, a 32 px da borda. O
conteúdo da tela desce o tamanho dessa linha (44 px): os retângulos de
`[achievements]`, `[saves]` e `[shots]` começam 44 px mais abaixo e
terminam onde terminavam, então qualquer posição que o tema der se ajusta
sozinha, nos três tamanhos de tela.

- Conquistas com `progress = "ring"`: o título ("Conquistas") fica ao lado
  do anel, no lugar do nome do jogo, e nada desce.
- Saves com `style = "preview"`: a imagem encolhe quando a altura não
  basta para ela e para a nota embaixo; a nota nunca é cortada.
- Busca com `[search] style = "field"`: sem cápsula nem relógio; o campo
  sobe para o topo (16 px) com "Cancelar" na cor de destaque à direita
  dele (com `field` em retângulo, o campo fica onde o tema puser, sem
  "Cancelar"). Na busca simples, "‹ <sistema>" na cápsula substitui o
  título.

Sem a tabela as telas ficam exatamente como sempre.

### Perguntas: `[dialog]`

```toml
[dialog]
width = 0.625         # largura (tamanho, fração de 480; padrão 1 = 480 px; 300 px)
radius = 0.054        # cantos (26 px)
icon = true           # um círculo com o ícone de alerta acima do texto, centralizado
buttons = "row"       # "row": as respostas em botões lado a lado; "hints": as dicas, como sempre
style = "glass"
```

Com `buttons = "row"` a resposta do A fica à direita, na cor de destaque;
as outras à esquerda. Vale para as perguntas das Configurações (esquecer um
aparelho, apagar, confirmar) e para as que aparecem sobre as telas de jogos:
apagar ou carregar um save, apagar uma captura, levar os saves para outro
emulador, trocar o driver de vídeo para o panfrost e o aviso dos relatórios.
Nessas, `width`, `radius` e `style` mudam o painel, e `buttons = "row"` troca
os botões pelos de cápsula.

A resposta que destrói ou reinicia (apagar, esquecer, remover, reiniciar,
trocar o driver, converter por cima de saves) sai em vermelho (`danger`): o
botão inteiro quando está em foco, só a palavra quando não está.

A pergunta de apagar ou carregar um save e a de apagar uma captura (Saves
e Capturas, botão X ou A) viram um alerta centralizado quando o tema tem
`[dialog]`: a largura, os cantos e o vidro da tabela; com `icon = true`, a
lixeira (ou o Play) num círculo da cor da ação; a pergunta (16 px) e o que
acontece embaixo dela, centralizados; e Cancelar e a ação lado a lado. Com
`buttons = "row"` são cápsulas de 40 px: a ação sempre preenchida, em
vermelho (`danger`) quando apaga e na cor de destaque quando joga;
Cancelar no branco translúcido; o botão em foco ganha o contorno do
[`focus_ring`](#sombra-vidro-foco-movimento-e-gradiente) (sem ele, o
destaque em 2 px). Sem `[dialog]` a pergunta fica como sempre, com a
miniatura do save.

### Menu Start: `[start]`

```toml
[start]
style = "control"     # "tiles" (como sempre) ou "control": a central de controle
radius = 0.054        # cantos dos blocos (tamanho, fração de 480; 26 px)
shortcut = 0.108      # diâmetro dos atalhos redondos (52 px)
clock = 0.092         # tamanho da hora (44 px), ou o lugar com o tamanho dentro:
# clock = { x = 0.05, y = 0.05, w = 0.9, h = 0.14, size = 0.09 }
tiles = { x = 0.05, y = 0.25, w = 0.9, h = 0.28 }      # os três blocos lado a lado, da altura da caixa
shortcuts = { x = 0.05, y = 0.57, w = 0.9, h = 0.12 }  # os atalhos centrados na caixa
```

Sem `tiles`, `shortcuts` e `clock` em caixa, a central se monta a partir do
topo como sempre; com elas, cada parte fica no seu retângulo (a rede e a
bateria se alinham à direita da caixa da hora; o nome do item em foco fica
logo abaixo dos atalhos).

Com `"control"` o Menu Start abre sobre a tela de onde foi chamado, desfocada
uma vez e escurecida: a hora grande no alto à esquerda com "Arch R" embaixo, a
rede e a bateria em texto à direita (seguem `battery` e `radios` de
`[status]`), os blocos Recentes, Jogos e Ferramentas (e os dos módulos) em
vidro com o ícone no canto e o nome embaixo, os atalhos em círculos e, abaixo
deles, o nome e o estado do item em foco (os jogos recentes, a biblioteca, a
rede, as atualizações). O item em foco fica branco com o texto escuro (cores
`text` e `background`); o bloco cresce com `focus_scale`. Wi-Fi e Bluetooth
ligados ficam na cor de destaque, Desligar tem o ícone vermelho (`danger`) e
os selos de atualização do sistema e das lojas continuam. Os widgets dos
módulos ficam numa faixa entre os blocos e os atalhos. Os atalhos, a ordem e os
botões são os mesmos do Menu Start de sempre; `area` continua valendo.

### Menu do jogo e menu rápido: `[menu]`

```toml
[menu]
style = "context"     # "list" (como sempre) ou "context": um cartão ao lado do item em foco
anchor = "focus"      # "focus" (ao lado do item em foco) ou "center"
width = 0.625         # largura do cartão (tamanho, fração de 480; 300 px)
row = 0.079           # altura de uma linha (38 px)
radius = 0.054        # cantos (26 px)
# rect = { x = 0.25, y = 0.1, w = 0.5, h = 0.8 }   # o cartão nesta caixa (largura dela, altura até a dela)
```

`rect` põe o cartão numa caixa fixa, por cima de `anchor` e `width`; a lista
rola quando não cabe na altura da caixa.

Com `"context"` o menu vira um cartão de vidro ao lado do item em foco (à
direita dele se couber, senão à esquerda, embaixo ou em cima; sem espaço, no
lado oposto da tela; aberto dos detalhes ou dos botões do destaque, ao lado do
botão em foco): o nome do jogo e "Sistema · ano" no alto; no menu de um
jogo, Jogar (claro), Favorito (a estrela âmbar quando já é favorito) e Detalhes
lado a lado, percorridos com ← → (↓ desce para as linhas, ↑ volta); depois as
linhas com o ícone à esquerda, o valor (núcleo, MangoHud, quantos saves), a
seta › quando abrem outra tela e as chaves de Pastas e Favoritos primeiro. As
ações do console (Suspender, Reiniciar, Desligar) e as da lista ficam sob um
título pequeno; Desligar e Tirar dos recentes saem em vermelho (`danger`). A
lista rola quando não cabe. As ações são as mesmas do menu de sempre, mais
Jogar e Favorito no topo.

### Detalhes do jogo em destaque: `[details]`

```toml
[details]
style = "hero"        # "panels" (como sempre, nos retângulos) ou "hero"
shelf = true          # a prateleira "Mais de <sistema>" embaixo
```

Com `"hero"` a tela de detalhes ocupa a tela inteira: a captura (ou a capa)
do jogo desfocada atrás, com véus de degradê na cor `background`; uma cápsula
de vidro "‹ Sistema" no alto à esquerda e o relógio à direita; o nome do jogo
grande em `font_bold` (até duas linhas, encolhe antes de quebrar), a linha
ano · gênero · jogadores · desenvolvedora (os últimos saem quando não cabe), a
descrição (rola como sempre), os botões Jogar (claro), Favorito e Buscar de
novo (redondos; o em foco ganha o contorno de `focus_ring`) e "Jogado há …". A
captura fica à direita, com sombra; embaixo, as capas dos jogos vizinhos do
mesmo sistema. Título, descrição, botões e "Jogado há" formam um bloco
centrado entre a cápsula e a prateleira: numa tela mais alta (720x720) as
capas da prateleira param de crescer e o espaço vai para o destaque, sem
abrir buraco entre a descrição e os botões. Os retângulos `shot`, `info`,
`title`... não valem nesse estilo. Os botões e as dicas do rodapé são os de
sempre.

### Busca: `[search]`

```toml
[search]
style = "field"       # "plain" (como sempre) ou "field"
row = 0.079           # altura de um resultado (tamanho, fração de 480; 38 px)
height = 0.083        # altura do campo (40 px)
field = { x = 0.0375, y = 0.1, w = 0.925, h = 0.083 }    # o campo (padrão: sob o cabeçalho)
results = { x = 0.0375, y = 0.2, w = 0.925, h = 0.3 }    # a caixa dos resultados
dock = "bottom"       # "bottom" (padrão): teclado na borda de baixo; "results": logo abaixo dos resultados
```

Com `"field"` o campo vira uma cápsula de vidro com a lupa, o texto, o cursor
e "N de TOTAL" dentro, contornada na cor de destaque enquanto se digita; os
resultados ficam num grupo de vidro, cada um com a capa, o nome com o termo
buscado em `font_bold` e o sistema (ou a pasta) à direita; o teclado fica
encaixado embaixo, numa folha de vidro. As teclas seguem `[keyboard]`.

Sem `results`, os resultados vão de logo abaixo do campo até o teclado, e as
linhas crescem até 40 % numa tela mais alta, para cinco delas ocuparem o
espaço sem deixar um vão. Com `dock = "results"` o teclado fica num cartão
de vidro logo abaixo da caixa dos resultados (ou de cinco linhas, sem
`results`) e não muda de lugar enquanto se digita. Em letra pequena (texto de
11 px ou menos na tela), o negrito do termo fica tão fino quanto o resto, e
o resto do nome passa para `text_secondary`, para o termo continuar à
frente.

### Teclado na tela: `[keyboard]`

```toml
[keyboard]
sheet = true          # a edição numa folha de vidro
key = { h = 0.075, gap = 0.0125, radius = 0.025 }   # altura, espaço e cantos das teclas (padrão 34, 6 e 8 px)
```

Com `sheet = true` a edição de texto das Configurações (senha do Wi-Fi, contas,
cores...) fica numa folha de vidro com "Cancelar" à esquerda, o nome do campo
no meio e a confirmação ("Conectar", "Confirmar") à direita, a legenda e a
regra, o campo em cápsula e as teclas; nas teclas, a em foco fica clara (cor
`text`, letra `background`), a de confirmar na cor de destaque e as outras
em vidro. B continua cancelando e START confirmando, como dizem as dicas.
`key` vale também sem a folha, e para o teclado da busca.

A folha tem a altura do que leva (título, campo e teclas) e fica encaixada
embaixo, sobre a página de onde veio, escurecida; numa tela mais alta ela
não estica, a página continua visível acima. O Ellegance aumenta as teclas em
`[aspect.square.keyboard]` para a tela quadrada.

### OSD de volume e brilho: `[osd]`

O indicador de volume e brilho (e os cartões de bateria, relatório e disco)
é desenhado pelo `archr-osd`, um processo à parte que fica por cima de tudo,
jogos inclusive. Ele não lê o `theme.toml`: ao iniciar e sempre que aplica a
aparência (troca de tema ou de qualquer opção de Aparência), o launcher grava
o tema ativo já resolvido em `/run/archr-launcher/osd-theme.toml` (arquivo
temporário + rename, nunca pela metade). O OSD relê o arquivo quando ele
muda, na próxima vez que algo aparece; sem o arquivo, ou com ele quebrado,
desenha como sempre.

```toml
[osd]
style = "capsule"     # "pill" (padrão) ou "capsule"
position = "right"    # "right" (padrão), "left", "top" ou "bottom"
fill = "#F5F5F7"      # cor do nível (padrão #F1F3F7)
track = "#1C1C22A6"   # fundo atrás do nível (padrão: o do estilo)
radius = 0.0375       # cantos, em tamanho de 480 (pill 16 px, capsule 18 px)
```

- `pill`: a pílula escura de 44x136 px (em 640x480) com o nível claro subindo
  de baixo, o ícone embaixo, a 24 px da borda. É o desenho de sempre.
- `capsule`: cápsula vertical maior (50x200 px), de vidro escuro translúcido
  com borda clara de 1 px, brilho no topo e sombra; o nível claro sobe de
  baixo e o ícone fica embaixo, escuro sobre o nível e claro sobre o vidro.
- `position`: centrado na borda escolhida; em `top` e `bottom` o indicador
  fica deitado e o nível cresce da esquerda para a direita, com o ícone à
  esquerda.
- `fill` e `track` aceitam `#RRGGBB` ou `#RRGGBBAA`. Sobre um `fill` escuro o
  ícone fica claro.

Os cartões do OSD (bateria baixa e crítica, relatório, disco, sem salvar
estado) usam as cores do tema: `surface` (o cartão), `border`, `badge` (a
barra da contagem), `text`, `text_secondary`, `muted`, `accent`, `success`,
`warning`, `danger` e `scrim` (o véu da bateria crítica), e as fontes `font`
e `font_semibold`. O arquivo só leva as cores que diferem do tema padrão: o
padrão do OSD é o próprio Grafite, então um tema que não muda uma cor deixa
o OSD como sempre. Uma fonte que não carrega volta para a Inter do tema
padrão.

Para ver os estilos no PC: `ARCHR_OSD_STYLES=<pasta> cargo test -p archr-osd
-- --ignored styles` grava o volume em pill e capsule, em cada borda, sobre
um fundo cinza de 640x480.
Sobre telas do launcher, com o arquivo que um tema gera:
`ARCHR_OSD_THEME_OUT=/tmp/osd.toml cargo test -p archr-launcher
ellegance_gives` grava o do Ellegance, e `ARCHR_OSD_LOOK=/tmp/osd.toml
ARCHR_OSD_SCENES=<pasta de PNGs> ARCHR_OSD_OUT=<pasta> cargo test -p archr-osd
-- --ignored over_screens` desenha o volume a 60 % sobre cada PNG.

### Configurações: `[settings]`

Até a 3.0 as Configurações só mudavam cores, fontes e ícones com o tema. Desde
a 3.1 a tabela `[settings]` muda também o layout. Sem ela (ou com os valores
padrão) as Configurações ficam exatamente como sempre.

```toml
[settings]
layout = "sidebar"    # "list" (padrão): a lista de sempre; "sidebar": barra lateral
sidebar = { x = 0.022, w = 0.334, style = "glass", title = 0.046 }
row = 0.075           # altura de uma entrada da raiz, tamanho de 480 (padrão 36 px)
icon = "tile"         # "plain" (padrão) ou "tile": ícone branco em quadrado colorido
groups = "inset"      # "flat" (padrão) ou "inset": linhas em grupos arredondados
title = "below"       # "beside" (padrão): título ao lado da cápsula; "below": numa linha própria

[settings.icon_colors]  # cor do quadrado de cada página (só com icon = "tile")
network = "#0A84FF"
sound = "#FF375F"
```

- `layout = "sidebar"`: a raiz vira um painel à esquerda com o título
  "Configurações" em negrito e uma entrada por página; à direita, o resumo da
  página em foco (título grande e a descrição num grupo). ↑↓ andam na barra,
  A ou → abrem a página, que ocupa a tela inteira com "‹ <página de cima>"
  numa cápsula no topo e o título em negrito ao lado; B volta para a barra
  com o foco onde estava. Numa página, ← e → continuam mudando valores, como
  L e R. As dicas ficam na cápsula do [`[hints]`](#dicas-de-botões-hints),
  sem a linha do rodapé.
- `sidebar`: `x` e `w` em frações da largura de 640 px das Configurações
  (padrão 14 e 214 px), `style` `"glass"` (padrão, o vidro do tema) ou
  `"plain"` (a superfície com borda; vale também para a cápsula "‹"), e
  `title` o tamanho do título, fração de 480 (padrão 22 px). Com mais
  páginas do que cabem, as entradas ficam mais juntas.
- `icon = "tile"`: o ícone de cada entrada da raiz fica branco num quadrado
  arredondado de 26 px. Cores padrão: Rede `#0A84FF`, Som e tela `#FF375F`,
  Controles `#30B050`, Jogos `#FF7A00`, Região e idioma `#5E5CE6`, Aparência
  `#BF5AF2`, Launcher `#30B0C7`, Conquistas `#D98E04`, Sistema `#6E6E76`,
  Armazenamento `#1E9BD7`. Chaves de `[settings.icon_colors]`: `network`,
  `sound`, `controls`, `games`, `region`, `appearance`, `launcher`,
  `achievements`, `system`, `storage`; a página que o tema não cita fica com
  a cor padrão.
- `groups = "inset"`: em todas as páginas, as linhas seguidas sem título de
  seção entre elas formam um grupo arredondado (raio 14 px) no preenchimento
  do vidro do tema (sem vidro, o texto bem fraco), com uma regra fina entre
  as linhas; os títulos de seção ficam em maiúsculas cinza acima do grupo.
  Os interruptores ficam verdes (`success`) quando ligados, com o botão
  branco. A lista de escolhas (painel ao lado da linha e a página de
  escolhas longas) segue o mesmo desenho: cantos de 14 px e regras entre as
  escolhas. Linhas novas de qualquer página entram nos grupos sozinhas.
  No Wi-Fi, o interruptor e a rede conectada formam um grupo, "REDES" vem
  em cima das outras (com um indicador girando enquanto a busca roda), e
  Serviços de rede, Bluetooth e Adaptadores USB ficam num grupo à parte.
- `title = "below"` (só com `layout = "sidebar"`): o título da página fica
  grande (28 px, `font_bold`, com `title_tracking`) numa linha própria
  embaixo da cápsula "‹ <página de cima>", alinhado com o texto das linhas.
  A página inteira desce o tamanho dessa linha (36 px) e termina onde sempre
  terminou, então qualquer página de linhas, inclusive as novas, se ajusta
  sozinha. As páginas montadas até o rodapé (loja de temas e de módulos,
  atualizações, informações, BIOS, atalhos, teste do controle, coleta de
  artes, conquistas, teclado) mantêm o título ao lado da cápsula.

Com `layout = "sidebar"` ou `groups = "inset"`, a linha em foco deixa de ser
preenchida pela cor de destaque: fica um painel claro translúcido com o anel
do [`focus_ring`](#sombra-vidro-foco-movimento-e-gradiente) (sem ele, o
destaque em 2 px), e o texto dela mantém `text`. Com qualquer chave de
`[settings]` fora do padrão o resto do estilo do tema passa a valer nas
Configurações: `corners`, `border_width`, sombra (`shadow_color`,
`shadow_blur`, `shadow_offset`), vidro, `easing` e `motion = "spring"`. Os
títulos usam a fonte `font_bold`.

O tema de teste `prateleiras` usa a barra lateral, os quadrados e os grupos;
`ARCHR_SETTINGS_THEMED_DIR=<pasta> cargo test -p archr-ui --lib
the_sidebar_theme` grava a raiz, Wi-Fi, Serviços de rede, Jogos, Som e tela e
as duas listas de escolhas em 640x480, 720x720 e 480x320.
O mesmo para o Ellegance: `ARCHR_ELLEGANCE_CAPTURE_DIR=<pasta> cargo test
-p archr-ui --lib ellegance_draws_settings` (a raiz, Jogos, Serviços de rede,
Wi-Fi buscando e vazio, a senha e a lista de núcleos em 640x480 são referências
em `tests/golden/ellegance-640x480`). Com `ARCHR_SETTINGS_EVERY=1` o mesmo
teste desenha todas as páginas, para revisar, e cada página longa rolada até a
última linha (`config-<página>-scrolled-*`).

### Grade de jogos com título

Numa visualização com [prateleiras](#prateleiras), a grade de jogos
(`list.mark = "grid"`) usa o título grande `home.heading`: o nome do sistema
em `font_bold` e a posição ("4 de 213") à direita, no lugar da linha do
cabeçalho.

## Detalhes do jogo

Uma visualização com `[[view.<id>.details.el]]` troca esta tela por uma
tela livre (veja [Tela de detalhes](#tela-de-detalhes)); o que segue vale
para as outras.

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

## Apps de módulos

Um módulo com a API 4 pode ter **um app**: uma linha própria no carrossel
(`@app:<id do módulo>`, depois das coleções) que abre telas inteiras do módulo
(`docs/modulos.md`, seção 10). Todo tema já funciona com apps; a tabela
`[app]` (opcional) desenha as telas deles.

- **Arte no carrossel:** `systems/app-<id>.png` (ou `.jpg`, `.webp`), com as
  mesmas regras das imagens de console (a pasta do usuário, depois o tema,
  depois o padrão); sem ela, a arte do próprio módulo; sem nenhuma, o ícone
  do módulo em tamanho grande, no lugar do nome gerado.
- **Lista do app** (`layout = "collection"`): sempre uma lista de linhas com
  a imagem do item em foco, qualquer que seja a visualização: uma grade ou as
  prateleiras nunca levam a imagem embora. Sem `[app]`, é o layout clássico de
  jogos do tema (a lista à esquerda, a imagem à direita, a linha de informação
  sob ela), com o vidro, os cantos e o foco do tema. Os
  chips de filtro ficam numa faixa no topo da lista, nas cores do tema
  (`highlight_bg` no escolhido, `surface` e `border` nos outros); os selos dos
  itens usam `success`, `danger`, `muted` e a mistura de `highlight_bg` com
  `text`.
- **Detalhe do app** (`layout = "detail"`): os retângulos `details_*` desta
  seção acima, com a imagem do módulo em `details_shot` e os botões do módulo
  em `details_actions` (a barra de progresso preenche o botão).
- Teclado da busca, pergunta (`screen.confirm`) e indicador de espera são os
  do launcher, nas cores do tema. Texto nunca abaixo de 10 px, como em tudo.

### `[app]`: as telas dos apps

As mesmas chaves de uma tela de jogos de uma visualização (`header`,
`divider`, `list`, `panel`, `art`, `meta`, `backdrop`, `decor`), por cima do
layout clássico de jogos do tema. A lista é sempre vertical: `mark = "grid"`
ou `"tile"` e `axis = "horizontal"` viram linhas (com um aviso no log), e
`strip` não vale aqui. `header = "large"` (só em `[app]`) põe o nome da tela
em letra grande sob o relógio, com a posição à direita, como o título da
grade das prateleiras. As imagens dos itens são feitas no tamanho de `art`.
Vale `[aspect.<formato>.app]`.

```toml
# Ellegance: uma página de loja. O catálogo em linhas à esquerda, a imagem
# do item grande à direita com a linha dele embaixo.
[app]
header = "large"
divider = false
list = { x = 0.05, y = 0.215, w = 0.5, h = 0.665, row = 0.0833, gap = 0.0083, font = 0.03, radius = 0.025 }
panel = { x = 0.585, y = 0.29, w = 0.365, h = 0.41, radius = 0.0375, style = "glass" }
art = { x = 0.585, y = 0.29, w = 0.365, h = 0.41, fit = "cover", radius = 0.0375 }
meta = { x = 0.585, y = 0.72, w = 0.365, h = 0.05, size = 0.027 }
backdrop = false
```

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

### `[achievements]`: anel e grupo de vidro

```toml
[achievements]
progress = { x = 0.05, y = 0.11, w = 0.9, h = 0.18, style = "ring" }
list = { x = 0.05, y = 0.32, w = 0.9, h = 0.56 }
style = "glass"       # a lista num grupo arredondado de vidro
row = 0.1             # altura de uma linha (tamanho, fração de 480; padrão 50 px)
gap = 0               # espaço entre linhas (padrão 2 px)
badge = 0.075         # lado do selo (padrão 36 px)
badge_radius = 0.02   # cantos do selo (padrão 8 px)
```

`progress` é o retângulo, a palavra (`progress = "ring"`, no retângulo
herdado) ou os dois (`style` dentro do retângulo); `"bar"` é a barra de sempre.
Com `"ring"`: um anel de progresso com "2/7" dentro, o nome do jogo grande em
`font_bold`, a contagem e os pontos e, no modo hardcore, a cápsula "Hardcore"
na cor `warning`; dê ao retângulo altura para o anel (até 84 px). Com
`style = "glass"` (ou com o anel) as conquistas bloqueadas mostram um cadeado
no lugar do troféu e o selo baixado aparece esmaecido; a linha em foco ganha
um fundo claro e as outras são separadas por fios.

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

### `[saves]`: a prévia grande

```toml
[saves]
style = "preview"     # "list" (como sempre) ou "preview"
preview = { x = 0.05, y = 0.12, w = 0.5, h = 0.72 }
list = { x = 0.58, y = 0.12, w = 0.37, h = 0.72 }
row = 0.117           # altura de uma vaga na lista (tamanho; padrão 56 px)
thumb = 0.117         # largura da miniatura (padrão 56 px, 4:3)
```

Com `"preview"` a imagem do save em foco fica grande no retângulo `preview`
(4:3, cantos de 16 px, sombra), com o nome da vaga em `font_bold` e "Salvo em
… · há N dias" sobre um véu no pé dela, e o que acontece ao jogar logo abaixo.
As vagas ficam num grupo de vidro no retângulo `list`, cada uma com a
miniatura, o nome e há quanto tempo; a em foco ganha um fundo claro. Quando o
retângulo não tem altura para a imagem e para a nota inteira, a imagem
encolhe (até a metade da altura), para a nota nunca ser cortada.

### `[shots]`: a grade por data

```toml
[shots]
sections = true       # agrupa em "Esta semana" e "Antes", cada grupo começando uma linha
columns = 4           # colunas (0: quantas couberem, como sempre)
gap = 0.025           # espaço entre as imagens (padrão 12 px)
radius = 0.025        # cantos (padrão 8 px)
```

Com `sections` as capturas dos últimos 7 dias ficam sob "Esta semana" e as
outras sob "Antes" (títulos em `font_bold`); ↑ ↓ andam pela mesma coluna e
passam de um grupo ao outro.

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

### `[switcher]` em profundidade e `[resume]` como título

```toml
[switcher]
cards = "depth"       # "row" (como sempre) ou "depth"
card = { x = 0.22, y = 0.17, w = 0.56, h = 0.52 }
info = { x = 0.0, y = 0.74, w = 1.0, h = 0.14 }

[resume]
style = "title"       # "picture" (como sempre) ou "title"
progress = { x = 0.3, y = 0.9, w = 0.4, h = 0.008 }   # onde fica a barra da contagem
backdrop = { blur = 0.075, opacity = 0.5 }            # a imagem do jogo atrás
```

Com `cards = "depth"` o cartão do jogo selecionado fica no retângulo `card`,
com sombra e o contorno de `focus_ring`, e acima dele "Jogando agora" (o jogo
de onde se veio) ou há quanto tempo foi deixado; os dois vizinhos de cada lado
ficam menores (78 % e 60 %), esmaecidos e saindo pelas bordas da tela; o
fundo é a imagem do jogo selecionado desfocada e escurecida. Nome, sistema e
os pontos ficam em `info`. `side` não vale nesse estilo.

Com `style = "title"` a retomada ao ligar mostra, no meio da tela e sobre a
imagem do jogo desfocada, "Voltando para", o nome grande em `font_bold`, o
sistema e quando foi salvo, uma barra fina com a contagem e os dois botões.
`shot` e `info` não valem nesse estilo.

`progress` põe a barra da contagem num retângulo (sem ele, embaixo do texto;
os botões continuam embaixo do texto). `backdrop` escolhe a imagem do jogo
atrás da tela: `blur` (tamanho, fração de 480; `0` deixa nítida) e `opacity`
(0 a 1, quanto ela aparece através do fundo). Sem `backdrop`, o estilo
`"title"` desfoca como o vidro do tema e o `"picture"` não põe imagem atrás.

## Movendo as ROMs

A tela que aparece no boot de manutenção que move as ROMs para a partição
exFAT (`archr-launcher --roms-migrate-screen`) usa sempre o tema padrão
instalado, nunca o tema escolhido: nesse boot a partição STORAGE, onde ficam
os temas do usuário, é desmontada para encolher, e uma fonte aberta ali a
impediria. O layout é fixo. Cores do padrão: `accent`
no ícone, na barra e nos pontos, `border` no trilho, `text` no título,
`muted` no aviso, `text_secondary` na linha do andamento e `dim` no rodapé.

## Animações

Aparência › Animações (`launcher.animations`): **Ligadas** (`on`), **Reduzidas**
(`reduced`: só fades curtos, nada se desloca nem cresce) e **Desligadas**
(`off`: tudo instantâneo). Os valores antigos `1` e `0` valem como Ligadas e
Desligadas. Os tempos são fixos: seleção 180 ms, sistemas→jogos 220 ms, troca de
capa 200 ms, menus 160 ms, avisos 200 ms, interruptores 150 ms, abertura 300 ms.
O tema escolhe só a curva (`easing`, ver [Estilo](#estilo)) ou troca a curva
por uma mola (`motion = "spring"`, que assenta em 0,35 s; ver
[Sombra, vidro, foco, movimento e gradiente](#sombra-vidro-foco-movimento-e-gradiente)).
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
min_launcher = "0.2.0"       # versão mínima do launcher; abaixo dela não instala
```

`min_launcher` compara com a versão do **pacote** do launcher (hoje `0.5.0`),
não com a versão do sistema que as Configurações mostram (`v3.1.0 · …`) nem
com o número da API de temas (3.1). Um valor acima da versão do launcher
instalado faz a Loja recusar o tema ("Este tema pede uma versão mais nova do
launcher."): use a versão de que o tema precisa (tabela abaixo)
ou deixe a chave de fora.

| `min_launcher` | o launcher que a partir dele tem |
|---|---|
| `0.1.0` | temas até a API 3.1 (cores, fontes, visualizações, telas livres fases 1–8) |
| `0.2.0` | update 20261126.6: `tint` em `frame`, `default_view`, ladrilhado em uma cópia, `${system}` de app pelo id do módulo |

Um tema que usa algo da tabela precisa declarar a versão da linha: um launcher mais antigo o desenharia quebrado, e com o `min_launcher` ele recusa a instalação. O `archr-es2theme` grava a versão do launcher em que converteu.

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

O teste `resolutions` confere, em 640×480, 720×720, 480×320, 1024×768,
1024×600 e 1280×720, cada tema com
`theme.toml` em `themes/` e cada visualização dele: lista abaixo do
cabeçalho, nada fora da tela, painel, imagem, texto e pontos sem cobrir a
lista, e os retângulos dos detalhes, saves, Alternador e retomada sem se
sobrepor. Para conferir um tema seu, copie a pasta para `themes/` do
repositório e rode:

```sh
cargo test -p archr-ui --test resolutions
```
