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
80 ms. Os mapeamentos vêm só do banco SDL GameController
(`SDL_GAMECONTROLLERCONFIG_FILE`, exportado pela sessão); o `es_input.cfg` não
é lido. Pads reconhecidos usam apenas os eventos GameController; os demais, o
fallback de botões/eixo/hat do joystick.

`ARCHR_LAUNCHER_ROOT` prefixa os caminhos absolutos, inclusive os caminhos de
ROMs do catálogo de sistemas, ROMs/mídias absolutos do gamelist (antes dos testes
de existência), `/usr/bin/runemu.sh`, `/run/archr-launcher` e
`/tmp/.system.cfg.lock`. Assim uma execução de teste não grava a trava nem o
handoff do host. As fontes padrão tentam primeiro o prefixo, depois as fontes
do host. Isso facilita testes, mas não é uma sandbox de segurança.

O catálogo de sistemas (XML no formato do `es_systems.cfg`, gerado pelo pacote
`virtual/emulators`) é o primeiro arquivo existente entre
`/storage/.config/archr/systems.cfg` (substituição do usuário),
`/usr/share/archr/systems.cfg` (do pacote) e, só durante a transição, os
caminhos antigos `/storage/.config/emulationstation/es_systems.cfg` e
`/usr/config/emulationstation/es_systems.cfg`; usar um destes gera um aviso
no log.
O tema vem de `launcher.theme` em
`/storage/.config/system/configs/system.cfg` (padrão `default`).

## Medição de abertura

`ARCHR_LAUNCHER_TRACE=1` emite checkpoints cumulativos até o primeiro present.
Meça em release, no mesmo display/renderer e com caches quentes.
Uma única thread prepara fontes em memória e surfaces de texto enquanto
SDL cria janela/EGL/renderer na thread principal. `worker join` marca a entrega;
`CPU surfaces upload` registra o envio ao renderer. SDL_ttf é inicializado antes
da thread e seus handles nunca são usados concorrentemente. Não há cache de
badges/sombras em disco. O primeiro present mostra cabeçalho e lista preparados,
reutilizando as métricas de texto calculadas no worker. O painel de sistemas
(texto, sombra e arte) e o rodapé aparecem nos quadros seguintes, sem mudar o
resultado final. A resolução das imagens só começa após o primeiro present,
no worker de capas; imagens ausentes são um resultado normal, sem aviso.

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
- Sistemas com emuladores usam o contrato do `runemu.sh`. Sistemas sem
  emuladores (`tools`, `moonlight`, `imageviewer`) executam o `<command>` do
  catálogo pelo mesmo `next`. Nomes de arquivo nunca entram no modelo: os
  valores são exportados em `ARCHR_ROM`, `ARCHR_BASENAME` e `ARCHR_SYSTEM`, e
  cada marcador vira uma referência à variável. Só três formas são aceitas:
  palavra inteira sem aspas (`foot %ROM%`), palavra inteira entre aspas duplas
  (`"%ROM%"`) e `--command="%ROM%"` (o weston-terminal a repassa a outro
  `sh -c`, que expande a variável herdada). Barra invertida, crase, `$(`,
  `((`, marcador logo após `-c`, `eval` ou `let`, marcador desconhecido ou
  qualquer outro contexto: recusado com
  `error.no_core`, e o modelo vai para o log.
- Falhas do jogo são lidas de `last-exit`, exibidas por alguns segundos e
  apontam para `/var/log/exec.log`, usado por `runemu.sh`.

## Tema Grafite

Instale a pasta `themes/default/` inteira em
`/usr/share/archr-launcher/themes/default/`, incluindo `fonts/`, `icons/`,
as duas licenças OFL das fontes e `icons/LICENSE` (Lucide, licença ISC).
Os TTF estáticos são dos releases oficiais Inter 4.1 e JetBrains Mono
2.304; não entram no binário. Sem os arquivos, permanece o fallback para
Liberation/DejaVu. Para experimentar o tema no PC com `ARCHR_LAUNCHER_ROOT`,
copie essa pasta para `ROOT/usr/share/archr-launcher/themes/default/`.

Os SVGs oficiais Lucide 0.468.0 em `icons/` permanecem inalterados. Um tema
personalizado pode substituí-los em `<tema>/icons/<nome>.svg`; em seguida,
o launcher tenta `ROOT/usr/share/archr-launcher/themes/default/icons/` e,
se ausente ou inválido, usa as cópias incorporadas no binário. Diferentemente
das imagens de console, não existe pasta de ícones substitutos em `/storage`
fora do tema ativo. Ícones são rasterizados no tamanho de exibição e tingidos
pela cor do tema; os do primeiro quadro de sistemas são preparados no worker.

Imagens de console da tela de sistemas: `/storage/.config/archr-launcher/systems/<nome>.{png,jpg,webp}`
tem precedência sobre `<tema>/systems/<nome>.{png,jpg,webp}`; sem imagem, o
painel mostra um cartão tipográfico gerado. `<nome>` é o `<name>` do
catálogo de sistemas; sistemas parentes (ex. `megadrive` e `genesis`) usam a mesma
foto por uma tabela de apelidos em `render/system_art.rs`.

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
abre Informações do sistema. O menu Start também abre as configurações.
Som e tela, Sistema e Região e idioma aplicam alterações imediatamente;
as outras categorias mostram “Em breve”. Atualização mostra a versão instalada,
sem iniciar o atualizador. O driver de GPU oferece confirmação de reinício.

As páginas são descritas em `archr-ui::settings`; o host usa
`archr-launcher::settings::Runtime`. Um worker serial executa scripts com argv,
limite de 20 s e código de saída real. Volume usa `pactl` diretamente e brilho
usa o backlight; após sucesso, `archr-config` publica a chave atomicamente sob a
trava compartilhada. Saída de áudio, governador, fuso, idioma e inatividade
também têm o launcher como único dono da chave. `gpudriver`, `suspendmode` e
`turbomode` são donos exclusivos das suas chaves: o launcher chama o script e
confere a gravação, sem escrever antes ou depois. Esses três scripts ainda
precisam de modo runtime-only e propagação de falhas no sistema; suas gravações
legadas não ganham atomicidade por serem chamadas daqui. A mesma propagação deve
ser conferida em `set-audio`, `archr-config tz` e nas funções de governador.
Erros exibem somente uma mensagem curta e localizada; detalhes técnicos
completos vão ao log. Leituras usam `archr-io`, rejeitando arquivos não regulares e limitando os
dados. Informações
são sondadas fora do SDL quatro segundos após a conclusão anterior; a saída
aguarda apenas mutações, nunca sondagens descartáveis. O driver de GPU ativo
é obtido pelo binding; a preferência divergente é marcada para após reiniciar.

As opções de suspensão por inatividade gravam `idle.timeout` e
`idle.timeout.game`, em minutos: 0, 1, 2, 5, 10, 15 ou 30; ambas começam em
0 (“Nunca”). O detector é o `input_sense`, que lê as chaves ao iniciar: o
launcher grava e reinicia a unidade `input` (se estiver ativa), como fazem os
quirks. Os atalhos nos jogos (Controles › Atalhos nos jogos) seguem o mesmo
caminho com as chaves `key.hotkey.a` e `hotkey.*` em nomes evdev; os botões
do próprio launcher ficam em `launcher.buttons`.
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
Antes do launcher, faz o que o `es_settings` fazia para o ES: executa
`/usr/bin/archr-apply-locale` (fuso e idioma) e importa no systemd as variáveis
listadas em `/etc/profile.d/999-export`. Os dois são opcionais e uma falha não
impede a UI. A sessão nunca espera a geração do locale: se o idioma configurado
ainda não existe em `/usr/lib/locale`, o script roda destacado na unidade
transitória `archr-apply-locale` (`systemd-run`, nice 19, E/S ociosa, uma
instância), e o launcher abre na hora com as próprias traduções. Gerar o locale
ao trocar o idioma cabe à ação de configurações. `LOCPATH` é removido: o glibc
acha os locales por `/usr/lib/locale`. Preserva os exports dos quirks/usuário e
configura Wayland e SDL. O banco de controles é o `SDL_GAMECONTROLLERCONFIG_FILE`
herdado só se o arquivo existir; senão o `gamecontrollerdb.txt` do usuário e,
sem ele, o de `/usr/config/SDL-GameControllerDB`.
A espera por sway reproduz a do `start_es.sh`: socket Wayland + IPC com saída
ativa, polling de 100 ms, teto de 10 s. Sem compositor pronto após o teto,
tenta iniciar a UI e deixa o backoff lidar com uma falha.

A janela usa o `app_id` Wayland fixo `archr-launcher` (o launcher define
`SDL_VIDEO_WAYLAND_WMCLASS` para si antes de iniciar o vídeo); as regras do sway
e do OSD devem usar esse nome.

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

## Moonlight

A coleção aparece quando `/usr/bin/moonlight` está instalado. Adicione o IPv4 do
PC; `A` abre/pareia/inicia, `X` atualiza e `Y` abre opções (salvas ao sair com
`B`/`Y`). Digite o PIN aleatório exibido no Sunshine; `B` cancela. Desparear também
esquece o PC; se estiver offline, um aviso informa que o pareamento remoto pode
permanecer. Para encerrar o stream: Start + Select + L1 + R1 ou Ctrl+Alt+Shift+Q.

Até 16 PCs e as opções ficam em `/storage/.config/archr-launcher/moonlight.cfg`.
O CLI usa `/storage/.config/moonlight/moonlight.conf` e chaves em
`/storage/.cache/moonlight`. A resolução inicial acompanha o painel.
