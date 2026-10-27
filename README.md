# archr-launcher

Launcher SDL2 para ArchR Linux. Requer Rust 1.88+, SDL2, SDL2_ttf 2.20+ e
SDL2_image 2.8+. O renderer usa aceleração e vsync; somente o driver SDL
`dummy`, usado explicitamente em testes sem GPU, usa software.

## Executar no PC

Na raiz deste repositório:

```sh
ARCHR_LAUNCHER_ROOT=crates/archr-index/tests/data \
  cargo run -p archr-launcher -- --windowed
```

A janela tem 640×480. Sem `--windowed`, usa fullscreen desktop na resolução
nativa. Os arquivos `Demo` da fixture são vazios, não jogos executáveis; servem
para validar navegação, imagens e o contrato do supervisor, sem iniciar emuladores.

Teclado: Enter abre, Backspace volta, X favorita, Y abre o menu, PgUp/PgDn
ou esquerda/direita mudam de página/núcleo, cima/baixo selecionam. Escape fecha
somente em modo janela. Direções mantidas repetem após 300 ms e depois a cada
80 ms. Uma entrada de `es_input.cfg` com GUID ou nome correspondente tem
prioridade, inclusive para pads reconhecidos como SDL GameController: nesses
pads só os eventos raw são usados, sem duplicar os eventos GameController.
Sem entrada ES, usa SDL GameController ou o fallback de botões/eixo/hat.

`ARCHR_LAUNCHER_ROOT` prefixa os caminhos absolutos, inclusive os caminhos de
ROMs de `es_systems.cfg`, ROMs/mídias absolutos do gamelist (antes dos testes
de existência), `/usr/bin/runemu.sh`, `/run/archr-launcher` e
`/tmp/.system.cfg.lock`. Assim uma execução de teste não grava a trava nem o
handoff do host. As fontes padrão tentam primeiro o prefixo, depois as fontes
do host. Isso facilita testes, mas não é uma sandbox de segurança.

A configuração de sistemas vem de
`/storage/.config/emulationstation/es_systems.cfg`, ou de
`/usr/config/emulationstation/es_systems.cfg` se a primeira não existir.
O tema vem de `launcher.theme` em
`/storage/.config/system/configs/system.cfg` (padrão `default`).

## Navegação automatizada

A opção de depuração oculta `--script` exige `--windowed` e aceita uma sequência
separada por vírgulas: `A,B,X,Y,L,R,UP,DOWN,START,SELECT,WAIT,SNAPSHOT,QUIT`.
Ela aguarda a carga assíncrona dos jogos e as gravações pendentes do núcleo.
Após escolher um jogo, o processo encerra com sucesso e deixa uma linha
`game ...` em `ROOT/run/archr-launcher/next`; **não executa o comando**.

```sh
ARCHR_LAUNCHER_ROOT=crates/archr-index/tests/data \
  cargo run -p archr-launcher -- --windowed --script 'A,B,Y,B,A,A'
```

`SNAPSHOT` exige `ARCHR_LAUNCHER_SCREENSHOT=/caminho/frame.png` e grava o próximo
frame antes do present. Imediatamente após um botão terminal, como em
`A,A,SNAPSHOT`, captura o overlay “Abrindo…” antes do handoff e da saída.
Essa captura é a única opção de depuração que faz I/O
no desenho. Sem display, os mesmos fluxos podem rodar com
`SDL_VIDEODRIVER=offscreen` (ou `dummy`).

## Organização e caches

- `archr-ui`: tema, SDL, entrada, miniaturas e modelo de navegação compartilhado.
  `archr-launcher::state` reexporta o modelo para manter a API anterior.
- `archr-launcher`: caminhos, adaptação da biblioteca, workers e handoff.
- A primeira apresentação usa somente o overview em cache; sem cache, mostra
  os nomes de todos os sistemas, ainda sem contagens. Workers só iniciam após
  essa apresentação. O overview reutiliza contagens quando os fingerprints
  baratos (path/extensões/mtimes) não mudaram e só salva quando há alterações.
  A atualização não bloqueia a carga do sistema selecionado. As coleções aparecem
  pelas chaves do Store; jogos são resolvidos só ao entrar na coleção. Pedidos
  de sistema selecionado têm prioridade sobre resoluções de coleção enfileiradas.
- Favoritos/histórico do ES são importados uma vez por sistema, quando sua lista
  é carregada. As coleções preservam o sistema e índice reais de cada jogo.
- Miniaturas são decodificadas/redimensionadas em worker e salvas em
  `/storage/.cache/archr-launcher/thumbs/<hash-caminho>-<mtime>.png`. Um pequeno
  arquivo `.size` invalida a miniatura quando o tamanho do retângulo muda.
  Somente a seleção e até dois vizinhos de cada lado são pedidos; até 16
  texturas de capa e 128 texturas de texto ficam em memória.
- Favoritos/importações são gravados por um writer serializado que coalesce
  snapshots pendentes. O snapshot final é confirmado antes de publicar `next`
  em lançamento/ação; fechar a janela também aguarda a gravação final.
  Antes desse trabalho terminal, apresenta um overlay de status centralizado.
  Falhas na gravação/handoff cancelam a saída e aparecem no banner.
- Escritas de núcleo são serializadas em worker; emulador e núcleo são publicados
  juntos, sob uma única trava/rename do `system.cfg`. Um lançamento aguarda essas
  escritas sem bloquear o event loop.
- Falhas do jogo são lidas de `last-exit`, exibidas por alguns segundos e
  apontam para `/var/log/exec.log`, usado por `runemu.sh`.

## Tema Grafite

Instale a pasta `themes/default/` inteira em
`/usr/share/archr-launcher/themes/default/`, incluindo `fonts/` e as duas licenças
OFL. Os TTF estáticos são dos releases oficiais Inter 4.1 e JetBrains Mono
2.304; não entram no binário. Sem os arquivos, permanece o fallback para
Liberation/DejaVu. Para experimentar o tema no PC com `ARCHR_LAUNCHER_ROOT`,
copie essa pasta para `ROOT/usr/share/archr-launcher/themes/default/`.

Todos os tokens de `theme.toml` são opcionais, com fallback individual.
`accent`, `accent_text` e `dim` também aceitam os nomes antigos `highlight_bg`,
`highlight_text` e `dim_text`; os novos nomes têm precedência. Fontes continuam
restritas a arquivos dentro da pasta do tema. Raios/alturas/margens escalam pela
altura da tela; o retângulo da capa e a largura da lista usam frações conforme
os comentários do tema. O menu mantém o posicionamento óptico da maquete.

A bateria usa `ROOT/sys/class/power_supply/battery/{capacity,status}`, quando
há prefixo; sem capacidade válida, o indicador fica oculto. A leitura ocorre
no timer, no máximo a cada 30 s; o relógio usa a hora local e atualiza por
minuto. Seleção/rolagem animam por 90 ms e capas por 120 ms. Nova entrada
conclui os alvos anteriores imediatamente. Somente durante essas animações
há apresentações contínuas com vsync; parado, o desenho continua por eventos.

Ano (`releasedate`), jogadores (`players`) e último uso do gamelist aparecem
sob a capa quando disponíveis. O índice de jogos passa à versão 3 para esses
metadados; caches anteriores são descartados e refeitos pelo worker.

## Verificação

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

O teste de janela usa SDL offscreen e cobre navegação, captura de frame,
favorito, troca de núcleo, consumo de `last-exit` e publicação de `next`.
Os testes puros cobrem miniaturas por caminho/mtime, limites de cache, bindings,
coleções e resolução de núcleos. A meta de 0,3 s precisa ser medida no RK3326;
uma medição no PC não substitui a sonda no aparelho.
