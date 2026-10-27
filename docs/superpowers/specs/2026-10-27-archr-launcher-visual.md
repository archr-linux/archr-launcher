# archr-launcher: especificação visual (tema padrão "Grafite")

Data: 2026-10-27. Estado: direção aprovada pelo Douglas a partir das maquetes.

A primeira versão funcionava, mas sem acabamento. O launcher precisa ser rápido **e** bonito; este documento fixa o visual do tema padrão e o que o motor de desenho precisa oferecer para chegar nele.

## Identidade

Segue a identidade do sistema e do site arch-r.io (DESIGN.md do site): uma única cor de destaque forte sobre neutros escuros, tipografia dupla (sem serifa para a interface, monoespaçada para números e dados técnicos). Sem gradientes coloridos, sem arco-íris de categorias.

### Cores (tokens do tema padrão)

| Token | Valor | Uso |
|---|---|---|
| background | `#0B0F14` | fundo |
| surface | `#11161C` | cartões, área da capa |
| surface_elevated | `#161C23` | menu rápido |
| border | `#1C232C` | divisórias, bordas de cartões |
| badge | `#1C2530` | círculo dos botões |
| text | `#E6EDF3` | texto principal |
| text_secondary | `#C5CFDA` | itens perto das bordas da lista |
| muted | `#9BA7B4` | barra superior, dicas, metadados |
| dim | `#6B7785` | contagens, itens longe da seleção |
| accent | `#1E88FF` | item selecionado, estrela de favoritos |
| accent_text | `#FFFFFF` | texto sobre o destaque |
| accent_text_secondary | `#CFE4FF` | contagem sobre o destaque |
| scrim | `rgba(5,8,11,0.60)` | fundo escurecido sob o menu rápido |

### Tipografia

- Interface: **Inter** (400, 600). Números, relógio, bateria, contagens e "4/213": **JetBrains Mono** (400/500). Ambas OFL; os TTF vão em `themes/default/fonts/` com as licenças e são instaladas em `/usr/share/archr-launcher/themes/default/fonts/`.
- Tamanhos de referência a 480 px de altura (escalam pela altura): título da barra 15, itens 17 (selecionado 18, peso 600), contagens 13 mono, barra e dicas 13, letras dos botões 11 peso 700.
- Texto longo é cortado com reticências, nunca ultrapassa a coluna.

## Layout (640x480, tudo em frações para escalar)

- **Barra superior** 44 px: à esquerda "ARCHR" (sistemas) ou o nome do sistema + posição `4/213` (jogos); à direita relógio e ícone de bateria com porcentagem (lidos de `/sys/class/power_supply/battery/{capacity,status}`, ícone de raio quando `Charging`). Margem lateral 24.
- **Lista**: margem 16, itens de 38 px (selecionado 44 px nos sistemas, 42 nos jogos), cantos 10 px. O selecionado é uma pílula preenchida com `accent`. Itens distantes da seleção esmaecem (text → text_secondary → dim) nas bordas da área visível. Sistemas mostram a contagem de jogos à direita (mono, dim). Coleções no topo com ícone (estrela para Favoritos, relógio para Recentes) e divisória fina depois delas.
- **Jogos**: lista à esquerda; à direita coluna de 210 px com a capa (cantos 12 px, borda 1 px `border`, sombra suave) e, abaixo, uma linha de metadados (ano · jogadores à esquerda, "há 3 dias" à direita) quando o gamelist tiver. Sem capa: cartão `surface` vazio com o nome do sistema em `dim`, nunca um bloco de cor chapada. Favorito marcado com estrela no item.
- **Barra inferior** 48 px com divisória superior: à esquerda um resumo ("9 sistemas · 612 jogos") na tela de sistemas; à direita as dicas de botão: círculo de 20 px `badge` com a letra **centralizada pela caixa real do glifo** (não pela linha de base), seguido do rótulo.
- **Menu rápido**: a tela de trás continua desenhada e recebe o `scrim`; um cartão centralizado de 340 px, cantos 16, `surface_elevated`, borda `border`, sombra. Cabeçalho com nome do jogo e sistema (muted). Item "Núcleo" mostra `L <núcleo> R`. Divisória antes de Suspender/Reiniciar/Desligar.
- **Avisos** ("Abrindo…", erros): cartão no mesmo estilo do menu rápido, centralizado.

## Movimento

- A pílula de seleção desliza até o novo item (cerca de 90 ms, ease-out) e a lista rola de forma suave quando a seleção sai da área visível. A animação nunca atrasa a entrada: um novo botão salta para o destino.
- Durante animações o laço desenha a 60 Hz; parado, volta a só desenhar quando algo muda.
- Capas entram com fade de 120 ms quando a miniatura chega.

## Motor de desenho (o que o archr-ui precisa)

- Retângulos com cantos arredondados preenchidos e com borda (via `SDL_RenderGeometry`, SDL 2.0.18+; o aparelho tem SDL 2.32).
- Sombra suave: textura de sombra pré-gerada uma vez (9-slice), sem blur por quadro.
- Ícones simples (estrela, relógio, bateria, raio) desenhados por geometria ou pequenas texturas geradas na inicialização; nada de arquivos de ícone obrigatórios.
- Texto com Inter/JetBrains Mono via SDL2_ttf, cache de texturas por (texto, fonte, tamanho, cor); reticências medidas com a fonte.
- Tudo isso continua barato: a meta de primeira tela < 0,3 s vale com o tema novo.

## Tema

O `theme.toml` ganha os tokens acima, `font`, `font_mono`, raios (`radius_item`, `radius_card`) e as alturas; cada campo continua com fallback individual para o padrão embutido. Temas antigos (só com os campos da v1) continuam válidos.
