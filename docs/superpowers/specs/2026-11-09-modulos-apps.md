# Módulos API 4: apps no carrossel, e o app de jogos do itch.io

Estado: proposta (2026-11-09). Pedido do Douglas: "os módulos [devem] permitir
criar esse tipo de app", com um item no carrossel como o Moonlight, e um app
nosso para baixar jogos do itch.io. O
[NextUI-Itchio-Pak](https://github.com/carroarmato0/NextUI-Itchio-Pak) (MIT)
serve de referência de fluxo e de como o itch.io responde; nada do código dele
entra no ArchR.

## 1. O que muda

Hoje um módulo acrescenta pedaços às telas do launcher (bloco e atalho no
Start, item no menu Y, páginas em Configurações). A API 4 deixa um módulo ter
**um app**: um item próprio no carrossel que abre telas inteiras do módulo,
navegáveis como as do launcher (lista com capas, detalhe com imagens,
botões com progresso, busca, avisos), e que pode **instalar jogos** na
biblioteca.

## 2. Princípio: o módulo descreve, o launcher desenha

A regra dos módulos continua valendo: **o módulo nunca desenha pixels nem
lê o controle**. Um app manda documentos JSON que dizem o que a tela tem; o
launcher desenha com os componentes dele.

Por que não um programa com tela própria (como o Pak, em SDL2):

- **Isolamento:** um programa gráfico precisaria do Wayland, do controle e do
  áudio dentro do sandbox. Com o launcher desenhando, o módulo continua sem
  tela, sem controle e sem acesso à biblioteca.
- **Tema e resoluções:** as telas do app saem com o tema, a paleta, a fonte e
  o tamanho de texto do usuário, em 640×480, 720×720 e 480×320, sem nada no
  módulo.
- **Controle e acessibilidade:** botões, repetição, atalhos, teclado e textos
  em quatro idiomas já existem; cada app não reinventa.
- **Custo:** o módulo é um processo pequeno (Rust ou sh+jq); a parte pesada
  (decodificar imagens, desenhar, animar) é do launcher, que já faz isso para
  capas.

O preço: o app só tem os componentes que o launcher oferece. A seção 5 lista
o conjunto inicial, pensado para catálogos e lojas, que é o que esse tipo de
app faz.

## 3. Manifesto

```toml
id = "itchio"
api_version = 4
mode = "daemon"            # obrigatório para apps (ver 4)
net = true

[app]
label = "@app.name"        # "Jogos do itch.io"
subtitle = "@app.subtitle" # linha abaixo do nome no carrossel
icon = "store"             # Lucide, como os demais pontos de extensão
art = "art/app.png"        # opcional: imagem do carrossel (1:1, até 512 px)
home = "catalog"           # id da primeira tela

[permissions]
install = ["gb", "gbc", "gba", "nes", "megadrive", "pico-8"]
launch = true              # "Jogar" depois de baixar (já existe na API 2)
```

- **Um `[app]` por módulo.** O item vira uma linha do carrossel como um
  sistema: entra depois das coleções, pode ser movido e ocultado (Y › Mover,
  Ocultar sistema), e a ordem fica salva. Id da linha: `@app:<id do módulo>`.
- **Arte:** a do módulo; o tema pode trocar pondo `systems/app-<id>.png`
  (mesma regra das artes de sistema). Sem arte, o ícone grande.
- **Contagem:** a linha de resumo ("1.240 jogos") vem de `ui.update` para
  `app:summary`.

### Permissões novas (mostradas na Loja antes de instalar)

| permissão | o que libera | texto na Loja |
|---|---|---|
| `[app]` | telas próprias, abertas pelo carrossel | "Tem telas próprias" |
| `install = [...]` | `rom.install`/`rom.remove` nesses sistemas, só para jogos que o próprio módulo instalou | "Instala jogos em: Game Boy, …" |

`net` e `launch` continuam como hoje. Não há permissão de escrita em pastas: a
instalação passa pelo launcher (seção 6).

## 4. Execução

- O app é um **daemon que só vive enquanto o app está aberto**: o launcher o
  inicia ao abrir o item do carrossel e o encerra (`shutdown`, 500 ms, mata)
  ao sair do app, ao abrir um jogo e ao suspender. Não entra no limite de 3
  daemons, porque só um app fica aberto por vez.
- Se o módulo também precisa de um daemon permanente (blocos no Start), são
  dois processos: o de sempre e o do app (`ARCHR_MODULE_ROLE=app`).
- Sandbox igual ao de hoje (seção 3 de `docs/modulos.md`). Recursos do
  processo do app: `MemoryMax=96M`, `TasksMax=32` (downloads em paralelo).
- **Downloads continuam se o app fechar?** Não na fase 1: sair do app cancela.
  O módulo guarda o que já baixou em `HOME/downloads/` e retoma (Range) na
  próxima abertura.

## 5. Telas

O módulo abre telas com `screen.show` e altera partes com `screen.patch`. O
launcher mantém a pilha: B volta à tela anterior e, na primeira, sai do app.
Cada tela tem `id`, `title` (cabeçalho) e um `layout`.

### 5.1 `layout = "collection"`: catálogo

Lista de itens com imagem, como a lista de jogos de um sistema (usa o layout
`games` da visualização do tema: lista à esquerda, capa à direita, ou a
grade/vitrine que o tema definir).

```json
{"method":"screen.show","params":{
  "id":"catalog","title":"@title.catalog","layout":"collection",
  "chips":[{"id":"all","label":"@all","selected":true},{"id":"gb","label":"GB"}],
  "items":[{"id":"g123","title":"Tobu Tobu Girl","subtitle":"Game Boy · tangram games",
            "badge":"@badge.free","tone":"good","image":"cache/covers/g123.png"}],
  "more":true,
  "buttons":{"y":{"label":"@search","action":"search"},"x":{"label":"@sort","action":"sort"}},
  "footer":"@footer.count"}}
```

- A num item: `screen.select {screen, item}`; o módulo responde abrindo outra
  tela (detalhe).
- `chips`: filtros no topo; L/R trocam (`screen.chip {screen, chip}`).
- `more: true`: ao chegar perto do fim o launcher manda `screen.more`; o
  módulo acrescenta com `screen.patch {append: [...]}` (páginas de até 50).
- X e Y: ações do módulo (`action.invoke`, como hoje); SELECT abre a busca
  (teclado do launcher → `screen.search {screen, text}`); START abre as
  páginas de ajustes do módulo (Configurações, já existentes).

### 5.2 `layout = "detail"`: um item

Usa o layout de detalhes de jogo do tema: imagem grande, galeria, título,
linha, tabela nome/valor, texto com rolagem e botões. É a página de detalhe da
API 3 com imagens do próprio módulo.

```json
{"method":"screen.show","params":{
  "id":"game:g123","title":"Tobu Tobu Girl","layout":"detail",
  "image":"cache/covers/g123.png",
  "gallery":["cache/shots/g123-1.png","cache/shots/g123-2.png"],
  "subtitle":"Game Boy · tangram games · 2017",
  "facts":[{"title":"@price","value":"@free","tone":"good"},{"title":"@size","value":"128 KB"}],
  "text":"Tobu Tobu Girl is an arcade platformer…",
  "buttons":[{"id":"get","label":"@download","icon":"download","action":"download"},
             {"id":"page","label":"@qr.page","icon":"qr-code","action":"show-qr"}]}}
```

- Botão com progresso: `screen.patch {target:"button:get", progress: 42,
  label:"@downloading"}`; ao terminar vira "Jogar" (`action: "play"`).
- L/R passam a galeria.

### 5.3 Componentes de apoio

| mensagem | desenho |
|---|---|
| `screen.toast {text, tone}` | faixa de aviso (a mesma do launcher), sem o limite de 10 s de `ui.notify` dentro do app |
| `screen.confirm {id, text, yes, no}` | diálogo de confirmação do launcher; resposta `screen.answer {id, yes}` |
| `screen.qr {title, value, text}` | tela com código QR desenhado pelo launcher (para abrir uma página ou entrar na conta no celular) |
| `screen.busy {text}` | indicador de carregamento na tela atual, até a próxima `screen.show`/`patch` |
| `screen.offline {text}` | selo "Sem conexão" no cabeçalho |

### 5.4 Imagens

- Só arquivos dentro de `HOME` do módulo (`module-data/<id>`), por caminho
  relativo. O launcher recusa `..`, links simbólicos para fora, arquivos acima
  de 4 MiB e formatos que não sejam PNG, JPEG ou WebP.
- O launcher decodifica e reduz ao tamanho de uso no carregador de capas (o
  mesmo dos fundos de sistema), fora da thread de desenho. O módulo baixa as
  imagens ele mesmo (tem `net`).

### 5.5 Textos

`@chave` resolve no catálogo do módulo, como hoje. Textos vindos da rede (nome
e descrição de um jogo) vão como texto puro; o launcher não interpreta HTML nem
formatação.

## 6. Instalar jogos

```json
{"id":7,"method":"rom.install","params":{
  "system":"gb","file":"downloads/tobu.gb","name":"Tobu Tobu Girl",
  "cover":"cache/covers/g123.png",
  "meta":{"developer":"tangram games","description":"…","year":2017,"source":"https://tangramgames.itch.io/tobutobugirl"}}}
→ {"result":{"game_key":"gb/Tobu Tobu Girl.gb"}}
```

O launcher:

1. confere o sistema contra `install` e a extensão contra as do sistema;
   arquivo de até 64 MiB; `.zip` só se o sistema aceita zip;
2. dá um nome seguro (sem `/`, `..`, caracteres de controle; colisão ganha
   ` (2)`) e copia para a pasta de ROMs do sistema (o cartão onde a pasta
   estiver, como uma cópia pela rede);
3. copia a capa para `media/images/` e grava `name`, `desc`, `developer`,
   `releasedate` no `gamelist.xml` (o mesmo editor do raspador);
4. registra o jogo como do módulo (`module-data/installed.toml`, fora do
   alcance do módulo) e relê o sistema; o jogo aparece na lista na hora.

`rom.remove {game_key}` só remove jogos que esse módulo instalou (ROM, capa e
entrada do gamelist), depois de `screen.confirm`. `rom.list` devolve os jogos
que o módulo instalou e ainda existem (`game_key`, `system`, tamanho, data),
para o app marcar "baixado" e "atualização disponível".

O módulo nunca lê a pasta de ROMs nem a biblioteca: só sabe o que ele mesmo
instalou.

## 7. Segurança (o que a revisão deve olhar)

- Nada novo dentro do sandbox: o app não ganha tela, controle nem pastas.
- Superfícies novas, todas no launcher: caminhos de imagem e de arquivo
  (canonicalizar dentro de `HOME`, recusar links), tamanho e formato de
  imagens, nome e extensão do jogo instalado, quantidade de mensagens (as
  telas juntam patches a 4 por segundo, como `ui.update`).
- `rom.install`: no máximo 1 por segundo; total de 2 GiB por módulo
  (`rom.list` informa o uso).
- Conteúdo de terceiros: o app mostra o que o serviço publica. Um app que
  pode mostrar conteúdo adulto deixa a escolha ao usuário num ajuste próprio,
  desligado por padrão e ligado só depois de confirmação de idade; a Loja exige
  o aviso "não oficial" quando o app usa um serviço de terceiros.
- Selo "Revisado pelo ArchR" continua sendo por pacote publicado.

## 8. Tema

As telas do app usam layouts que o tema já tem (`games` da visualização ativa
para `collection`, `details_*` para `detail`), então todo tema já funciona com
apps. Novidades para autores de tema, a documentar em `docs/temas.md`, no site
e na skill: a arte `systems/app-<id>.png` no carrossel e a aparência dos
chips de filtro (cores do tema, sem chave nova na fase 1).

## 9. O app "Jogos do itch.io" (módulo oficial)

Binário aarch64 em Rust (rede, HTML e JSON em sh+jq ficariam frágeis), no
repositório `archr-linux/archr-module-itchio`.

**Catálogo** (do que o Pak documenta sobre o itch.io, reescrito por nós):

- Listas por plataforma pelos feeds RSS do itch.io
  (`https://itch.io/games/<filtro>.xml?page=N`), com as tags e ferramentas de
  cada plataforma: GB Studio para GB/GBC, homebrew de GBA, NES, Mega Drive e
  PICO-8, mapeadas para os nossos sistemas (`gb`, `gbc`, `gba`, `nes`,
  `megadrive`, `pico-8`).
- Detalhe pela página do jogo (descrição, capturas, preço, autor). Página
  HTML muda sem aviso: o parser fica num só lugar, com testes sobre páginas
  salvas, e falha de forma visível ("não foi possível ler este jogo") em vez
  de baixar errado.
- Cache de capas e de listas em `HOME/cache`, com validade, para abrir
  rápido e funcionar sem rede.

**Download de jogos grátis:** o fluxo do botão "Download" do site (token CSRF
da página, `download_url`, lista de arquivos, arquivo do CDN), escolhendo só os
arquivos com extensão do sistema. Depois, `rom.install`.

**Jogos pagos (fase 3):** entrar na conta pelo celular com `screen.qr` e a
chave da API do usuário; lista os jogos comprados (`api.itch.io`).

**Telas:** Catálogo (chips: Todos, GB, GBC, GBA, NES, Mega Drive, PICO-8;
ordem: Populares, Novos, A-Z), Busca, Detalhe (Baixar, Jogar, Remover, QR da
página), Ajustes.

**Ajustes** (páginas de Configurações do módulo, abertas por START dentro do
app ou por Configurações › Sistema › Módulos):

| linha | padrão | efeito |
|---|---|---|
| Conteúdo adulto | desligado | ligado mostra jogos que o autor marcou como adultos no itch.io; para ligar, o app pede `screen.confirm` "Tenho 18 anos ou mais e quero ver conteúdo adulto" |
| Violência e terror | ligado | jogos marcados com violência intensa ou terror |
| Jogos pagos | ligado | lista também jogos à venda (comprar pelo QR no celular) |
| Sistemas mostrados | todos | quais chips aparecem no catálogo |
| Ordem | Populares | ordem inicial do catálogo |
| Apagar capas e listas guardadas | — | limpa `HOME/cache` |

Diferente do Pak, nenhum filtro é imposto: o usuário escolhe cada um, e só o
conteúdo adulto vem desligado. O filtro vale no catálogo, na busca e nas
atualizações. A classificação vem das marcações dos autores no itch.io e pode
faltar; a tela de ajustes diz isso.

## 10. Fases

1. **Launcher (API 4):** `[app]` no carrossel (mover, ocultar, arte, resumo),
   ciclo do processo do app, `collection` e `detail` com imagens, chips, busca,
   botões com progresso, toast, confirmação, `rom.install`/`list`/`remove`,
   permissões e textos na Loja, `--check-module` para API 4. Exemplo
   `examples/modules/catalogo-demo` (catálogo local, sem rede) com testes.
   Documentação: `docs/modulos.md` seção 10, `docs/temas.md`, site, skill.
2. **Módulo itch.io:** catálogo grátis por plataforma, detalhe, download e
   instalação, "baixado"/"atualização", filtros de conteúdo, busca, cache.
   Revisão de segurança e selo; publicação na Loja.
3. **Extras:** `screen.qr` e jogos pagos; downloads que continuam com o app
   fechado (se fizer falta).

## 11. Decisões para o Douglas

- Nome do app na tela: "Jogos do itch.io" ou "itch.io".
- Sistemas na primeira versão: os seis acima, ou só GB/GBC (onde há mais jogos
  homebrew prontos).
- Apps de terceiros: liberar a API 4 para qualquer módulo na Loja desde o
  início, ou só para módulos revisados até a API amadurecer.
