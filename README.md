# archr-launcher

Launcher SDL2 para ArchR Linux. Requer Rust 1.88+, SDL2, SDL2_ttf 2.20+ e
SDL2_image 2.8+. O renderer padrão é GLES2 com aceleração e vsync; o driver SDL
`dummy`, usado explicitamente em testes sem GPU, usa software. Um override
`SDL_RENDER_DRIVER` continua disponível para diagnóstico.

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

## Experimentos de abertura (temporários)

`ARCHR_LAUNCHER_EXP` é lida uma vez ao criar a UI. Ausente = todas as melhorias
ligadas; vazia ou `none` = todas desligadas; uma lista liga **somente** os nomes
informados. Nomes desconhecidos são erro. Remover estes controles após as medições.

| Nome | Ligado | Desligado |
| --- | --- | --- |
| `nowinrecreate` | Janela OPENGL com perfil ES 2.0 desde a criação | Janela SDL sem flag OPENGL |
| `gles2` | Hint `opengles2`, ES 2.0 sem depth/stencil/MSAA | Seleção padrão SDL |
| `fontshare` | Um handle TTF por arquivo físico; troca tamanho/estilo | Um handle por arquivo e tamanho |
| `badgecache` | PNG dos badges por tema/escala | Rasterização dos badges |
| `shadowcache` | PNG da sombra nine-slice por tema/escala | Rasterização da sombra |
| `lazyassets` | Apenas assets visíveis; sem SDL_image | Prepara A/B/X/Y e sombra, mede título e inicializa SDL_image |

```sh
# Todos ligados, com checkpoints cumulativos e uma linha com o conjunto ativo:
ARCHR_LAUNCHER_TRACE=1 archr-launcher
# Só janela correta + GLES2; os demais experimentos ficam desligados:
ARCHR_LAUNCHER_TRACE=1 ARCHR_LAUNCHER_EXP=nowinrecreate,gles2 archr-launcher
# Controle sem melhorias:
ARCHR_LAUNCHER_TRACE=1 ARCHR_LAUNCHER_EXP=none archr-launcher
```

Um `SDL_RENDER_DRIVER` explícito continua tendo precedência sobre o hint.
`dummy` não recebe flags/atributos GL. `nowinrecreate` configura também o perfil
ES; a garantia de não passar pelo `SDL_RecreateWindow` do GLES2 depende de usar
esse renderer (use `gles2` junto, ou force `SDL_RENDER_DRIVER=opengles2` nos dois
lados do A/B). O custo do contexto pode migrar do checkpoint renderer para janela;
compare sempre o tempo **até first present**, não só uma fase isolada.

Meça 30 aberturas por variante em release, mesmo display/renderer e caches quentes.
A/B de sombra deve omitir `lazyassets` ou medir a abertura do primeiro painel:
sem painel visível, o trabalho da sombra já não ocorre antes do primeiro present.
`fontshare` desligado mede a estratégia por tamanho; o HEAD 1aec48c já trocava
os tamanhos por face lógica, mas ainda não compartilhava aliases do mesmo arquivo.

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
  `/storage/.cache/archr-launcher/thumbs/<hash-caminho>-<mtime>-<largura>x<altura>.png`.
  As dimensões fazem parte da chave; tamanhos diferentes coexistem sem `.size`.
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
os comentários do tema. O menu mantém o posicionamento óptico da maquete e
reforça a opacidade do scrim em 20% (0,60 → aproximadamente 0,72), sem alterar
os avisos.

Os badges A/B/X/Y são compostos no primeiro uso, com círculo e letra
em 4× (80×80 → 20×20), Inter SemiBold real, centro pela caixa de tinta e
filtragem linear. Os PNGs dos badges e da sombra nine-slice ficam em
`/storage/.cache/archr-launcher/raster-<hash-tema-escala>-<asset>.png`, também
sob `ARCHR_LAUNCHER_ROOT`. A chave inclui versão do raster, tema, escala,
metadados das fontes candidatas e, nos badges, letra, tamanhos e cores.
Ausência, PNG inválido ou dimensões erradas regeneram o asset; falha de escrita
mantém a UI funcionando sem persistência. PNGs usam o codec Rust, sem SDL_image.
A carga/gravação ocorre só no primeiro uso; quadros seguintes só copiam a textura.
No primeiro quadro normal, apenas A/Y são necessários; sombra/B/X esperam sua tela.
Temas personalizados usam o mesmo cache e invalidam pela própria configuração.
Temas sem peso próprio usam bold sintetizado também em 4×.

A bateria usa `ROOT/sys/class/power_supply/battery/{capacity,status}`, quando
há prefixo; sem capacidade válida, o indicador fica oculto. Um worker começa
depois da primeira apresentação e faz a leitura no máximo a cada 30 s; até
chegar a primeira amostra, a bateria fica oculta. O desenho só consulta um
snapshot tipado, sem bloquear nem esperar pelo sysfs. O relógio usa a hora local e atualiza
por minuto. Seleção/rolagem animam por 90 ms e capas por 120 ms. Nova entrada
conclui os alvos anteriores imediatamente. Somente durante essas animações há
apresentações contínuas com vsync; parado, o desenho continua por eventos.

Ano (`releasedate`), jogadores (`players`) e último uso do gamelist aparecem
sob a capa quando disponíveis. O índice de jogos passa à versão 3 para esses
metadados; caches anteriores são descartados e refeitos pelo worker.

## Verificação

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Benchmark manual da seleção no PC, com aquecimento e amostras a ~60 Hz:

```sh
cargo test -p archr-ui --test animation_timing -- --ignored --nocapture
cargo test --release -p archr-ui --test animation_timing -- --ignored --nocapture
```

Imprime mediana/p95 de `tick + draw + present` durante a animação, descartando
os quatro primeiros gestos. O backend offscreen não espera vblank; o intervalo
de amostragem de 16 ms fica fora da medição.

O teste de janela usa SDL offscreen e cobre navegação, captura de frame,
favorito, troca de núcleo, consumo de `last-exit` e publicação de `next`.
Os testes puros cobrem miniaturas por caminho/mtime, limites de cache, bindings,
coleções e resolução de núcleos. A meta de 0,3 s precisa ser medida no RK3326;
uma medição no PC não substitui a sonda no aparelho.

## Configurações

Entrada temporária: `Y` na lista de sistemas → `Configurações` → `A`.
`B` volta pela pilha de páginas; na raiz retorna ao launcher. `Y` na raiz
abre Informações do sistema. O futuro menu Start pode emitir `Outcome::Settings`.
Som e tela, Sistema e Região e idioma aplicam alterações imediatamente;
as outras categorias mostram “Em breve”. Atualização mostra a versão instalada,
sem iniciar o atualizador. O driver de GPU oferece confirmação de reinício.

As páginas são descritas em `archr-ui::settings`; o host usa
`archr-launcher::settings::Runtime`. Um worker serial executa scripts com argv,
limite de 20 s e código de saída real, e usa `archr-config` para gravações
atômicas sob a trava compartilhada. `suspendmode` é a exceção de ordenação:
o script precisa ver a chave antiga. Erros mantêm o valor anterior e mostram
o diagnóstico na página. Informações do sistema são sondadas fora do SDL
a cada quatro segundos enquanto a página está aberta.

As opções de suspensão por inatividade gravam `idle.timeout` e
`idle.timeout.game`, em minutos: 0, 1, 3, 5, 10, 15 ou 30; ambas começam em
0 (“Nunca”). O launcher apenas grava essas duas chaves: o detector de
inatividade é responsabilidade do sistema, inclusive durante jogos.
O idioma recarrega o catálogo do launcher após a gravação; os locales das
ferramentas e emuladores continuam dependendo da preparação feita pelo serviço.

Os testes usam `ARCHR_LAUNCHER_ROOT` e executáveis falsos em `ROOT/usr/bin`,
com esse diretório no `PATH`; nenhum script ausente pode cair no host.
O driver `SNAPSHOT` também aguarda as operações pendentes de configurações.

## Idiomas

Instale `locales/*.toml` em `/usr/share/archr-launcher/locales/` (também sob
`ARCHR_LAUNCHER_ROOT` nos testes de PC). A v1 inclui `en`, `pt_BR`, `es` e `ru`.
Os quatro catálogos ficam embutidos no binário: esses idiomas não consultam
arquivos de locale antes da primeira tela, mesmo quando há arquivos instalados.
Somente códigos que não resolvem para um idioma embutido consultam o diretório
externo (novos idiomas ou catálogos personalizados); esse caminho raro é síncrono.
O inglês embutido continua sendo a reserva final.
A configuração `system.language` é consultada no snapshot já carregado na abertura;
sem essa chave, usa `LANG`. Não há uma segunda leitura de configuração para idioma.
O código completo tem prioridade, seguido da língua e do inglês; `pt` também
resolve para o catálogo brasileiro, assim como `pt_PT`, `pt_AO` e demais regiões. Sufixos de `LANG` (`.UTF-8`, `@modifier`) são
ignorados, e hífen/underscore são aceitos no código. Uma chave ausente usa o inglês
embutido e, se também não existir nele, mostra a própria chave. O aviso é emitido
no máximo uma vez por chave durante a execução. Diagnósticos técnicos ficam no
log; os banners usam mensagens traduzidas. Uso, argumentos e erros do driver
`--script` são sempre em inglês e são processados antes de carregar configuração
ou catálogos.

Os catálogos são TOML plano: chaves pontuadas devem estar entre aspas, por exemplo
`"hint.open" = "Abrir"`. Para plurais, use uma tabela inline com categorias CLDR:
`"games.count" = { one = "{count} jogo", other = "{count} jogos" }`.
`other` é obrigatório; russo também usa `few` e `many`. Interpolação por `{name}`
não reinterpreta os valores inseridos. Arquivos precisam ser regulares, não podem
ser links simbólicos e têm limite de 64 KiB; falhas usam o próximo fallback.

Novos idiomas são descobertos pelo nome do arquivo, sem lista fixa no carregador.
Para acrescentar um idioma, copie todas as chaves do inglês, traduza os textos e
adicione a regra cardinal em `crates/archr-ui/src/i18n.rs` se necessário. Os testes
conferem chaves, categorias, placeholders, larguras das dicas/menu e cobertura
cirílica dos quatro TTF distribuídos. As traduções ainda precisam de revisão
por falantes dos idiomas antes de fechar a versão.

Francês, alemão, chinês simplificado e japonês ficam para a fase 2. A extensão para
uma fonte CJK lazy está marcada em `crates/archr-ui/src/text.rs`; nenhuma fonte CJK
é aberta agora. Tempo relativo usa meses aproximados de 30 dias e anos de 365 dias,
e datas futuras aparecem como “agora”.

## Instalação no ArchR

O pacote `projects/ArchR/packages/ui/archr-launcher/package.mk` no repositório
`distribution` compila para aarch64 com o cargo/rust do próprio build, ao lado
do EmulationStation. Instala os dois executáveis em `/usr/bin`, a unidade em
`/usr/lib/systemd/system`, o tema completo (quatro TTF e licenças OFL) e os
catálogos em `/usr/share/archr-launcher/`. A unidade **não é habilitada** pelo
pacote: o ES continua como padrão.

O download segue o padrão `archr-splash`: arquivo do GitHub fixado por commit.
O `PKG_VERSION` inicial é um placeholder de integração: antes de um build remoto,
substitua-o pelo hash publicado que inclui `session/` e este perfil de release.
Para desenvolvimento, a partir da raiz de `distribution`:

```sh
ARCHR_LAUNCHER_SOURCE_DIR=/caminho/absoluto/archr-launcher \
  PROJECT=ArchR DEVICE=RK3326 ARCH=aarch64 scripts/build archr-launcher
```

Esse override usa o suporte nativo `file://` do build para copiar um snapshot
da árvore local, sem `.git`, caches e `target/`; não usa `PKG_SOURCE_DIR`, que
apenas renomeia a pasta extraída. Os arquivos de código, sessão, temas e locales
entram no stamp de unpack, invalidando o snapshot quando mudam. Mantenha o mesmo
override nas etapas de build/instalação da imagem. Sem override, usa o commit
remoto. O código é GPL-2.0-or-later (texto em `LICENSE`); as fontes mantêm
suas licenças OFL independentes.

Depois de incluir o pacote na imagem, para optar pela sessão, altere **somente**
a linha `UI_SERVICE` em `/storage/.config/profile.d/090-ui_service`, preservando
as demais opções (inclusive `MALI_WAYLAND_AFBC`):

```sh
UI_SERVICE="sway.service archr-launcher-session.service"
```

Reinicie o aparelho para aplicar. Para voltar ao ES, restaure
`UI_SERVICE="sway.service essway.service"` e reinicie. Não habilite as duas UIs
simultaneamente e não use `systemctl enable` para contornar o seletor.

A sessão carrega `/etc/profile` uma vez em Bash, pois os profiles da distro
contêm funções Bash, e usa `exec` para substituí-lo pelo supervisor POSIX sh.
Preserva os exports dos quirks/usuário e configura Wayland, SDL e o locale já
gerado pelo ES, sem chamar `es_settings`: esse script também gera locales,
reinicia `tz-data`, importa o ambiente do systemd e escolhe o alvo do hotkey.
A espera por sway reproduz a do `start_es.sh`: socket Wayland + IPC com saída
ativa, polling de 100 ms, teto de 10 s. Sem compositor pronto após o teto,
tenta iniciar a UI e deixa o backoff lidar com uma falha.

O hotkey de saída (`input_sense`) lê `/tmp/.process-kill-data` e executa
`killall` com seu conteúdo. No menu, a sessão grava `archr-launcher`, nunca o
nome do supervisor. Antes do jogo, remove o alvo do menu; `runemu.sh` e seus
wrappers definem os processos reais (`retroarch retroarch32`, `-9 mupen64plus`,
etc.). Não definir `runemu.sh` como alvo: ele precisa sobreviver para restaurar
serviços, governador e demais estados. Seu `quit()` ainda grava
`emulationstation`; a próxima volta da sessão troca esse alvo por
`archr-launcher`. Nenhum desses scripts da distro foi modificado.

Antes de cada execução do launcher, a sessão descarta qualquer `next` antigo.
Só aceita `game ...` após saída com código 0; falhas descartam o pedido e aplicam
backoff de 1 s. Cada `next` é lido sem reinterpretar escapes e removido antes da
execução; `last-exit` recebe o código decimal via arquivo temporário + rename.

Suspender mantém o launcher aberto: após o flush, relê `get_setting system.suspendmode`
e inicia `archr-fake-suspend power` (off/ausente) ou `systemctl suspend` sem esperar.

Jogos rodam em um grupo separado com `setsid` (habilitado no BusyBox do ArchR).
Em TERM, INT ou HUP, o supervisor percorre os PPIDs em `/proc/*/stat` e sinaliza
primeiro somente os descendentes de `runemu.sh`. O wrapper e seu `sh -c` ficam
vivos para executar a limpeza normal após a saída do emulador, sem exigir trap
no wrapper. Aguarda até cerca de 4 s pelo grupo; se ainda houver processos,
envia TERM ao grupo e, após 200 ms, KILL. Sem `setsid`, acompanha o PID filho.
A unidade usa `KillMode=mixed` para que o primeiro SIGTERM atinja somente o
supervisor, não mate prematuramente o wrapper; `TimeoutStopSec=6` mantém o
SIGKILL de todo o cgroup como fallback final.

O log do supervisor, `/var/log/archr-launcher-session.log`, é truncado aos 256 KiB.
stdout/stderr dos filhos vão ao journal; o log do jogo continua em `/var/log/exec.log`.

Validação hermética no PC, sem serviços/dispositivos reais:

```sh
sh session/test-session.sh
SESSION_TEST_SHELL=dash dash session/test-session.sh
# BusyBox: aponte SESSION_TEST_SHELL para um symlink chamado sh para busybox.
SESSION_TEST_SHELL=/caminho/busybox-bin/sh busybox sh session/test-session.sh
```

Os doubles de launcher, runemu, systemctl e swaymsg ficam no PATH temporário.
As variáveis `ARCHR_SESSION_*` de caminhos, shell, profile, sleep e espera só são
consultadas com `ARCHR_SESSION_TEST=1`; não são configuração da sessão instalada.
Os testes também validam o bridge de profile Bash para sh e o alvo do hotkey;
com Python 3 disponível, criam sockets locais para testar sway pronto. Testes
com RetroArch, standalone, ports e medição da primeira tela continuam sendo
validações no aparelho, não substituídas pelos testes do host.
