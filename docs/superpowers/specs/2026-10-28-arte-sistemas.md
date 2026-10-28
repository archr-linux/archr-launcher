# Arte de sistemas para o archr-launcher: pesquisa de conjuntos candidatos

Data: 2026-10-28. Pesquisa somente leitura. Amostras em `/tmp/claude-1000/-media-disco-local-ArchR/4aaf2e6f-cc32-4024-87aa-c7ce177206ea/scratchpad/arte-sistemas/` (subpastas `ra/` e `el/`).

## Conclusão curta

**Nenhum conjunto pronto de arte retrato (~250x370) com licença livre existe.** Todos os conjuntos de arte decorativa por sistema (ES-DE Modern/Slate, Art Book Next, Epic Noir, Chicuelo, Retrorama, Carbon) são CC-BY-NC-SA ou "sem licença / proibido uso comercial", o que é inutilizável numa distro GPL (restrição NC é incompatível com redistribuição livre). Os conjuntos "MIT" (Elementerial, Pixel Perfect) usam capturas de tela de jogos comerciais como arte do sistema (vi FF6 no "snes" e FF7 no "psx"): a licença MIT do repositório não cobre esse material de terceiros.

A única fonte **claramente livre** e com cobertura ampla é o **libretro/retroarch-assets (CC-BY 4.0)**, mas são **ícones quadrados** (256x256 XMB, 40x40 Ozone), não retratos. Servem como elemento central de um cartão gerado.

**Recomendação:** cartão tipográfico/gerado pelo próprio launcher como padrão (sempre completo, 100% nosso, sem questão de licença), com ícone opcional dos "xmb/systematic" ou "xmb/monochrome" do retroarch-assets (CC-BY 4.0, atribuição no `CREDITS`) como camada central quando existir. Arte de usuário/tema substitui por cima.

## Tabela ranqueada (melhor primeiro)

| # | Nome | URL | Licença (linha-chave) | Cobertura top-40 | Formato | Estilo | Tamanho | Veredito |
|---|------|-----|----------------------|------------------|---------|--------|---------|----------|
| 1 | **Cartão gerado (tipográfico)** | (nosso) | GPL, nossa autoria | 125/125 | qualquer, renderizado em 250x370 | Grafite + acento azul, consistente | ~0 (gerado) | **Usável: primário** |
| 2 | **libretro/retroarch-assets** (xmb/systematic, monochrome, flatui, dot-art, pixel; ozone icons) | https://github.com/libretro/retroarch-assets | `COPYING`: "Attribution 4.0 International" (CC-BY 4.0); API GitHub: `CC-BY-4.0` | Muito ampla: ~850 a 1240 PNG por tema, nomes No-Intro ("Nintendo - Super Nintendo Entertainment System.png"); cobre todos os top-40 (nes, snes, n64, gb, gbc, gba, nds, md, sms, gg, segacd, 32x, saturn, dc, psx, psp, pce, neogeo, atari, lynx, ws, ngp, msx, amiga, c64, dos, scummvm, mame...) | XMB: 256x256 PNG (RGBA ou paleta); Ozone: 40x40 cinza+alfa; também SVG em `src/xmb/*` (escaláveis) | Ícone de console estilizado (systematic = flat colorido; monochrome = silhueta). Quadrado, **não** retrato; ficaria como ícone ~160 px sobre fundo Grafite | Repo inteiro 817 MB (com tudo); só `xmb/monochrome/png` (~853 arquivos) é pequeno; extrair 1 tema por tamanho | **Usável** com atribuição CC-BY. Ressalva de marca: os ícones representam hardware com logos (ex.: logo ATARI no wallpaper); fontes têm licenças próprias (DejaVu, Open Sans). Baixar só o subconjunto e manter `COPYING` |
| 3 | wallpapers "posterized consoles" (mesmo repo) | idem, `wallpapers/posterized consoles/1440x900/` | CC-BY 4.0 (mesmo repo) | só 21 sistemas (Atari 2600, Lynx, GB, GBC, N64, NDS, NES, SNES, MD, SMS, PCE, PSX, PSP, MAME, DOS...) | 2288x1430 PNG, paisagem, monocromático tingido (visto: Atari 2600 em vermelho escuro, com logo ATARI) | Foto posterizada de console; recorte vertical 250x370 viável como fundo escurecido | ~1 a 3 MB cada | Usável (cobertura parcial, estilo diferente do Grafite, carrega marcas) |
| 4 | Elementerial (assets/systems + logos) | https://github.com/mluizvitor/es-theme-elementerial | `LICENSE.md`: "MIT License / Copyright (c) 2021 mluizvitor" | 106/125 dos nossos nomes (nomes idênticos aos do es_systems.cfg: `megadrive-japan`, `gbah`, `snesmsu1`...). Faltam: bbcmicro bk chip-8 gbav gp32 idtech imageviewer ios macintosh megadriveh moto music ndsiware neocd palm pc-8800 pc-9800 pet st-v vircon32 windows | systems/*.webp 1920x960 (paisagem 2:1); logos/*.svg | `systems`: **screenshots de jogos comerciais** (FF6, FF7) com MIT apenas no repo | 7,2 MB systems + 0,8 MB logos | **Arte: inutilizável** (direitos de terceiros). **Logos SVG: possível**, mas com ressalva de marca registrada e sem garantia de que o autor os tenha desenhado; verificar um a um |
| 5 | ES-DE `carousel-icons` | https://gitlab.com/es-de/themes/carousel-icons | `LICENSE`: "MIT License / Copyright (c) 2023 szymon-kulak" | amplo (nomes ES-DE), pastas `images`, `nso`, `simplified` | ícones/PNG (não medi) | Ícones de carrossel | pequeno | Provavelmente usável após conferir `CREDITS.md` (não inspecionei imagem por imagem). Quadrado, não retrato |
| 6 | ES-DE `system-graphics-mini`, `-silver-ring`, `system-controllers-outline`, `system-logos`, `system-backgrounds` | https://gitlab.com/es-de/themes/ | **Sem LICENSE e README vazio** no repo (verificado via API) | muito ampla (nomes ES-DE, ~170 sistemas) | PNG / SVG | variado | pequeno a médio | **Inutilizável sem permissão** (sem licença = todos os direitos reservados). Pedir permissão ao autor (anthonycaccese/ES-DE) |
| 7 | ES-DE Modern / Slate | https://gitlab.com/es-de/themes/modern-es-de , slate-es-de | `LICENSE`: "PROHIBITED: - Commercial distribution ... Attribution-NonCommercial-ShareAlike 2.0 (CC-BY-NC-SA)" e "used logos and trademarks are copyright of their respective owners" | ampla | `art/` e pastas por sistema | limpo, moderno | ~dezenas de MB | **Inutilizável (NC)** |
| 8 | Art Book Next (o que o ROCKNIX usa) | https://github.com/anthonycaccese/art-book-next-es | README: "Creative Commons CC-BY-NC-SA" | ampla (`_inc/systems/artwork-*`) | PNG angulado/fullscreen, máscaras próprias | arte em livro, artwork "Noir" por tenlevels | 470 MB | **Inutilizável (NC)**. Observação: o `package.mk` do ROCKNIX diz `PKG_LICENSE="CUSTOM"` |
| 9 | Epic Noir | https://github.com/c64-dev/es-theme-epicnoir | README: "Creative Commons Attribution-NonCommercial-ShareAlike 4.0"; "PROHIBITED: Commercial distribution" | ampla | PNG/SVG por sistema | escuro, o mais parecido com Grafite | 61 MB | **Inutilizável (NC)** |
| 10 | Chicuelo | https://github.com/chicueloarcade/es-theme-Chicuelo | README: CC BY-NC-SA 4.0 (apesar de o GitHub marcar GPL-3.0) | ampla | PNG | escuro | 63 MB | **Inutilizável (NC)**, licença do repo contradiz o README |
| 11 | Retrorama Turbo | https://github.com/lipebello/es-theme-retrorama-turbo | GitHub: GPL-3.0, mas README: "It forbidden edit content without prior authorization and use in any kind commercial way" | ampla | PNG | colorido | 67 MB | **Inutilizável** (conflito de licença, proibição explícita) |
| 12 | es-theme-knulli (symbuzzer) | https://github.com/symbuzzer/es-theme-knulli | GPL-3.0, mas "Some logos used from ckau-book ... (Licensed under Attribution-NonCommercial-ShareAlike 4.0)" e "All logos, trademarks and photos copyright respective owners" | cobre `_inc/logos/<sistema>/logo.png` | logos PNG | simples (base es-theme-basic) | 138 MB | Parcialmente poluído por NC; **não usar sem separar o que é original** |
| 13 | LCARS p42 (prefect421) | https://github.com/prefect421/es-theme-lcars-p42 | GitHub: CC0-1.0 (não li o texto) | ampla | PNG | estilo Star Trek LCARS, não combina | 77 MB | Licença ok em tese, estilo inadequado e origem das imagens não verificada |
| 14 | Carbon (RetroPie) | https://github.com/RetroPie/es-theme-carbon | Sem licença declarada no repo (`license=null`) | ~50 sistemas, nomes de 2015 | SVG/PNG | escuro, neutro | 4 MB | **Inutilizável** (sem licença; logos de marcas) |
| 15 | Pixel Perfect (jessikarochas) | https://github.com/jessikarochas/es-theme-pixel-perfect | `LICENSE.md` MIT | não inspecionado | variado | pixel | 162 MB | Provável mesmo problema do Elementerial (imagens de terceiros); não adotar |
| 16 | NextUI / MinUI artwork packs | https://github.com/LoveRetro/NextUI e scrapers (josegonzalez, sinedied, Helaas: MIT, só código) | NextUI: `NOASSERTION`; os scrapers (MIT) só baixam arte de terceiros | NextUI usa **arte por jogo**, não arte por sistema | n/a | n/a | n/a | Nenhum pacote de arte de sistema com licença livre encontrado |

## ROCKNIX

- Repositório https://github.com/ROCKNIX/distribution, `LICENSE.md`: "ROCKNIX is a Linux distribution that is made up of many open-source components. Components are provided under their respective licenses. **This distribution includes components licensed for non-commercial use only.**" Branding do ROCKNIX: CC BY-NC-SA 4.0.
- O pacote de temas (`projects/ROCKNIX/packages/virtual/es-themes/package.mk`, `PKG_LICENSE="GPLv2"`) depende só de **es-theme-art-book-next** (`PKG_LICENSE="CUSTOM"`, fixado por hash). O tema, porém, é **CC-BY-NC-SA** no README do autor. O pacote remove as variantes `circuit`, `classic`, `nintendont`, `noir`, `outline` e deixa `artwork-*` restantes.
- Consequência: **não dá para copiar a arte do ROCKNIX**. Ele já mistura componentes NC. O ArchR (GPL) não deve fazer o mesmo.

## Convenção de nomes recomendada (pedido do coordenador)

**Chave de nome de sistema = o `<name>` do `es_systems.cfg` do ArchR** (os 125 de `/media/disco-local/ArchR/distribution/build.ArchR-RK3326.aarch64/image/system/usr/config/emulationstation/es_systems.cfg`). Verificação: esses nomes coincidem em 106/125 com o Elementerial (nomes estilo Batocera/ArkOS/EmulationStation-Next), então pacotes de tema ES clássicos/Batocera mapeiam quase 1:1; ES-DE usa nomes parecidos mas não idênticos (`genesis`, `gc`, `cdimono1`, `pcengine`), então precisa de tabela de alias.

Layout proposto (primeiro arquivo que existir vence, ordem de precedência):

1. Usuário: `/storage/.config/archr-launcher/systems/<name>.png` (também `.jpg`, `.webp`).
2. Tema ativo: `<tema>/systems/<name>.png` (opcional `<name>@portrait.png`/`<name>.svg`).
3. Padrão embutido: `<default-theme>/systems/<name>.png`.
4. Fallback: cartão tipográfico gerado.

Leitura de layouts existentes "como estão" (sem copiar arte; o usuário traz o próprio pacote):
- **ES clássico/Batocera/ArkOS (Elementerial, Carbon, Epic Noir etc.)**: `assets/systems/<name>.(webp|png)` ou `<tema>/<name>/art/system.png` + `<name>/theme.xml`. Dá para ler diretamente porque os nomes são os nossos. Pasta paisagem: usar "cover" e recortar centro para 250x370.
- **ES-DE**: `_inc/systems/artwork-*/<system>.png` (Art Book Next) ou `<tema>/<system>/...`; precisa tabela de alias de nomes.
- **RetroArch assets / Pegasus**: nomes No-Intro (`Nintendo - Super Nintendo Entertainment System.png`); precisa tabela `name -> nome No-Intro`, que o próprio ArchR já deve ter por causa do RetroArch (playlists).
- **NextUI/MinUI**: não consegui confirmar o layout de arte por sistema no repo (busca de código sem resultado). Pelo que lembro, a arte fica em `Roms/<Nome (TAG)>/.media/`, mas isso é **não verificado**.

Regras de renderização sugeridas: aceitar PNG/JPG/WebP/SVG; escala "cover" para 250x370 com recorte central; se a imagem for paisagem, mostrar com desfoque do mesmo quadro ao fundo; cache redimensionado em `/tmp` ou `~/.cache` para não decodificar WebP 1920x960 a cada seleção (RK3326).

## Cobertura e fallbacks

- Se usarmos retroarch-assets, os nomes dos 125 precisam de mapeamento para No-Intro; sem equivalentes (ports, tools, music, imageviewer, windows, ios, moto, idtech, gbah/gbav/gbch, megadriveh, nesh, snesh, genh etc., que são variantes "hack" e coleções) usar o ícone do sistema pai ou o cartão tipográfico.
- Cartão tipográfico: fundo Grafite com gradiente sutil, nome do sistema em fonte do tema, faixa/linha de acento azul, ano/fabricante opcional (o repo ES-DE `system-metadata` oferece descrições, mas sem licença verificada; não usar).

## Atribuição necessária se usarmos retroarch-assets

Incluir no pacote: o `COPYING` (CC-BY 4.0) e uma linha "Ícones de sistemas: libretro/retroarch-assets, CC-BY 4.0, https://github.com/libretro/retroarch-assets. Marcas citadas pertencem aos respectivos donos." Indicar alterações (redimensionamento/tingimento).

## Ações sugeridas

1. Implementar primeiro o cartão tipográfico + a cadeia de busca de arquivos acima (resolve 100% sem risco legal).
2. Opcional: baixar só `xmb/systematic/png` ou `xmb/monochrome/png` (ou os SVG de `src/xmb/monochrome`) do retroarch-assets, mapear os nomes, e usar como ícone central.
3. Se quiser arte retrato de verdade: encomendar/gerar arte própria (CC-BY-SA ou GPL) ou pedir permissão escrita aos autores do Art Book Next, Epic Noir ou system-graphics (ES-DE) para uso na distro.
