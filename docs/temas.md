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
| `list` | a lista | `x y w h`, `row`, `selected` (altura da linha escolhida), `gap`, `font`, `selected_font`, `radius`, `mark`, `scroll`, `fade` |
| `panel` | cartão em relevo (painel do sistema, moldura da capa) | `x y w h`, `radius` |
| `art` | imagem do console ou capa do jogo | `x y w h`, `fit`, `radius` |
| `backdrop` | a capa ao fundo da tela de jogos | `opacity`, `solid` (fração da largura em fundo puro), `edge` (opacidade do fundo na borda direita) |
| `info` | nome do sistema e detalhes | `x y w h`, `title`, `meta` (tamanhos), `align` (`left`/`center`/`right`), `valign` (`top`/`bottom`), `details` (`stacked`: fabricante, nome, contagem; `inline`: nome e "213 jogos · Nintendo · 1990") |
| `meta` | ano, jogadores e "jogado há" sob a capa | `x y w h`, `size` |
| `dots` | pontos de posição entre sistemas | `x y w h` |

Valores de `list`:

- `mark`: `"pill"` (faixa colorida), `"bar"` (barrinha ao lado do texto) ou
  `"tile"` (sistemas em blocos lado a lado, o escolhido grande no centro, com a
  imagem do console; só na tela de sistemas).
- `scroll`: `"edge"` (rola ao chegar na borda) ou `"center"` (mantém a escolha no meio).
- `fade`: tintas por distância da escolha, ex. `["text", "text_secondary", "muted", "dim"]`;
  a última se repete.

Um elemento opcional some com `false` (`panel = false`) e aparece com uma
tabela. Valores fora da faixa são limitados à tela, chaves desconhecidas e tipos
errados são ignorados com aviso no log; nada disso derruba o launcher.

### Criar uma visualização nova

1. Copie `themes/praia/` para `/storage/.config/archr-launcher/themes/meu/`.
2. Acrescente `[view.minha]` com um `name` e os elementos que mudam.
3. Em Aparência, escolha o tema **meu** e a visualização **minha**; ao sair da
   página a mudança vale na hora, sem reiniciar.

## Testar no PC

Captura das telas de sistemas, jogos (com e sem capa) e da Aparência, em
pt-BR e ru, para cada `pasta:visualização`:

```sh
ARCHR_SETTINGS_CAPTURE_DIR=/tmp/capturas \
ARCHR_CAPTURE_VIEWS=default:vitrine,praia:lista \
cargo test -p archr-ui --lib capture_views -- --ignored
```

O tema precisa estar em `themes/` do repositório para esse comando. No
aparelho, os avisos sobre chaves inválidas ficam no log do launcher.
