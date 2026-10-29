# Módulos do archr-launcher — fases 2 e 3

Data: 2026-10-29. Base: fase 1 (`docs/modulos.md`, API 1) no `feat/v1`.
Ramo: `feat/modulos-fase2`.

Decisões do dono que esta especificação segue: loja aberta, mais o selo
"Revisado pelo ArchR" (`archr-linux/archr-modules`); módulos só em shell + `jq`
ou binários (sem Python); nenhum módulo roda durante um jogo; um uid por módulo
(60600–61099) e sandbox `systemd-run`; backups inacessíveis; lista de
`config_read` aprovada e definitiva (as 8 chaves da fase 1).

Regra que vale para tudo abaixo: **o módulo pede, o launcher faz**. Toda
capacidade nova é uma operação intermediada (o launcher valida e executa) ou um
caminho a mais na sandbox, nunca um privilégio do processo do módulo. Cada
capacidade tem ameaça e mitigação; o que não tem mitigação boa fica fora.

Legenda do estado: **[feito]** implementado neste ramo, com testes;
**[aguarda]** especificado, espera a aprovação do dono (UI nova) ou um teste no
aparelho.

## 1. Modelo de ameaça

Quem ataca: o autor de um módulo da loja aberta (não revisado), ou quem tomou a
conta dele no GitHub. O usuário instala porque o módulo promete algo útil.

O que proteger, em ordem:

1. **root** e o resto do sistema (o launcher roda como root; `runemu.sh` e os
   scripts do sistema também);
2. **segredos**: `system.cfg` (senhas, tokens do RetroAchievements e do
   ScreenScraper, chave do Wi-Fi), backups (`/storage/roms/backup*`), chaves do
   Bluetooth, segredos de outros módulos;
3. **dados do usuário**: saves, savestates, capturas, ROMs (perda é o pior caso;
   leitura e envio para fora vem logo depois);
4. **o aparelho como produto**: abrir jogos, a bateria, a fluidez do launcher, a
   tela (golpe: módulo que finge ser o sistema).

Mitigações que já valem desde a fase 1 e não mudam: uid próprio; `/storage`
inteiro escondido e só o que foi concedido montado de volta; sem rede sem
`net`; `NoNewPrivileges`, sem capabilities; `SystemCallFilter=@system-service`
com `SystemCallArchitectures=native` (o kernel do RK3326 tem
`CONFIG_SECCOMP_FILTER=y`, então o filtro está ligado); memória, tarefas e CPU
limitadas; o módulo nunca desenha pixels nem lê o controle; tudo parado antes
do jogo.

**[feito]** Endurecimento novo, sem custo para módulos honestos:

- `InaccessiblePaths=-/run/dbus`: o barramento do sistema é `AF_UNIX`, que
  passava mesmo sem `net`; nenhum módulo fala com connman, bluez ou logind.
- Com `net = true`: `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6` (antes
  ficava sem restrição: netlink e pacotes crus passavam).

**[feito]** Módulos com `net`: `IPAddressDeny=localhost` (127.0.0.0/8 e ::1:
porta de comandos do RetroArch, sshd, Samba locais). DNS: o connman roda com
`--nodnsproxy` e o `/etc/resolv.conf` acaba em `/run/connman/resolv.conf`, com
os servidores do DHCP (não loopback), então o DNS segue funcionando. Se o
`resolv.conf` apontar para o stub do systemd-resolved (`127.0.0.53`/`.54`, que
só serve DNS), o launcher libera exatamente esse endereço
(`IPAddressAllow=`). `IPAddressAllow` não filtra porta; por isso um servidor em
`127.0.0.1` ou `::1` (que divide o endereço com outros serviços) **não** é
liberado: o módulo fica sem DNS e o launcher registra no log. A alternativa
completa para esse caso é o proxy de rede da fase 3 (o módulo fala só com um
socket do launcher, que resolve nomes por ele).

**[feito]** `InaccessiblePaths=-/run/systemd/userdb`: os sockets varlink do
userdbd. O systemd 255 resolve `User=` antes de montar o namespace
(`exec-invoke.c`: `get_fixed_user` antes de `apply_mount_namespace`), e o
nss-systemd, sem os sockets, lê direto os registros em `/run/userdb`, onde fica
o do módulo.

## 2. API 2

**[feito]** `api_version = 2` passa a existir. O launcher roda API 1 e API 2
(`api_ok`: de 1 até a mais nova). Um launcher antigo mostra o módulo API 2
como "precisa de um launcher mais novo", em vez de "inválido". Pedir qualquer
permissão nova com `api_version = 1` é erro fatal no manifesto.

`initialize.params.capabilities` agora lista também `config.set`,
`game.launch` e `launch.before`.

Novos códigos de erro: `-32003` limite de taxa, `-32000` falha do launcher
(gravar falhou). Os outros seguem iguais.

## 3. Permissões reservadas

Tabela-resumo (o texto de consentimento vai nos quatro idiomas; abaixo, pt-BR):

| permissão | escopo exato | sandbox | consentimento | sensível (âmbar) | estado |
|---|---|---|---|---|---|
| `config_write = [...]` | `config.set` só das chaves de `WRITABLE`, valores conferidos | nenhuma mudança | "Muda ajustes" · lista das chaves | sim, sempre | [feito] |
| `hooks = ["launch"]` | recebe o pedido `launch.before` | nenhuma | "Vê cada jogo que você abre" · "E pode ajustar essa abertura, até 0,3 s antes de começar" | não | [feito] |
| `system = ["cpu-profile"]` | em `launch.before`, `changes.governor` vale | nenhuma | "Escolhe o perfil de CPU de um jogo" · "Só naquela abertura; seu ajuste continua igual" | não | [feito] |
| `launch = true` | `game.launch` logo após uma ação do usuário | nenhuma | "Abre jogos" · "Só logo depois que você escolhe uma ação dele" | não | [feito] |
| `write = [...]` | gravação em `savestates` e `screenshots` | `BindPaths` + ACL do uid | "Grava seus savestates" … | sim, sempre | [aguarda teste] |

O token gravado em `modules.toml` (`granted`) é `config_write:<chave>`,
`hooks:launch`, `system:cpu-profile`, `launch`, `write:<alias>`. Uma
atualização que acrescenta qualquer token pede consentimento só para os novos
(`Permissions::without`, genérico para todas as listas).

### 3.1 `config_write` [feito]

Chaves aceitas (lista branca, definitiva para a fase 2):

| chave | valores | por que é segura | quem lê |
|---|---|---|---|
| `system.cpugovernor` | `performance`, `ondemand`, `schedutil`, `powersave` | o `runemu.sh` **executa o valor como nome de função** (`${CPU_GOVERNOR}`); por isso só a lista fechada de `099-freqfunctions` passa, nunca texto livre | `runemu.sh` em cada jogo |
| `global.netplay.nickname` | `[A-Za-z0-9 _.]`, 1–32, não só espaços (mesma regra do launcher) | o `runemu.sh` corta argumentos por substring; sem `-` não há como injetar opção | netplay |

Fora, e por quê: tudo com senha, token ou chave; Wi-Fi, SSH, Samba,
atualizações, senha de root (comprometem o aparelho); `system.timezone`,
`system.hostname`, `system.language` (identidade do aparelho e da UI: um
módulo não decide isso); volume e brilho (têm dono vivo — o caminho do OSD — e
voltam como ação de `system` na fase 3, não como chave).

Execução: pedido `config.set {key, value}` (só daemon) → o launcher confere a
permissão e o valor; se o valor é o atual, responde `{}` sem gravar nem
contar; senão confere a taxa (**1 por segundo e 10 por hora por módulo**,
senão `-32003`),
grava com `SystemCfg::set_and_save` (trava + publicação atômica, o mesmo dono
da chave que as páginas de Configurações usam), registra no log
("Module X set K to V") e avisa a biblioteca para reler o `system.cfg`. A
página de Configurações relê tudo ao abrir.

Ameaça: módulo troca o governador para drenar a bateria ou esquentar; troca o
apelido para se passar por outra pessoa no netplay. Mitigação: valores
fechados, consentimento em âmbar com as chaves listadas, log, taxa; o usuário
vê o valor novo nas Configurações e pode desligar o módulo.

### 3.2 `hooks = ["launch"]` e o pedido `launch.before` [feito]

Na hora em que o usuário abre um jogo, antes do `game.started`:

```json
{"jsonrpc":"2.0","id":7,"method":"launch.before",
 "params":{"game":{"system":"n64","game_key":"n64/mario.z64","name":"Super Mario 64"}}}
```

Resposta: `{"decision":"allow"}` ou
`{"decision":"modify","changes":{"governor":"performance"}}`.

- Só **daemons** prontos com `hooks = ["launch"]` recebem (um oneshot custa
  ~190 ms só para subir).
- **300 ms no total** para todas as respostas, em paralelo. Quem passa do prazo
  vale como `allow` naquela abertura, com aviso no log; **3 atrasos seguidos**
  contam uma falha (e 3 falhas em 10 min desligam). O jogo nunca espera mais
  que isso. Uma falha antiga (mais de 30 s) não impede o daemon de voltar
  depois do jogo.
- **Não existe `deny`**: um módulo não impede o usuário de jogar (golpe de
  "pague para liberar", travamento por bug).
- `changes` só tem efeito com a capacidade de `system` correspondente; sem ela
  a mudança é ignorada com aviso no log, sem falha. Chave desconhecida em
  `changes` é ignorada.
- Dois módulos mudando a mesma coisa: vale o primeiro em ordem de `id`; o
  outro vai para o log.
- Netplay e Moonlight não perguntam (não há jogo local).
- O pedido não traz o caminho da ROM (quem quer ler a ROM pede `read`).

Ameaça: módulo que vê cada jogo aberto (privacidade). Os eventos
`game.started`/`game.ended` já dizem isso desde a fase 1, sem permissão; o
texto de consentimento deixa explícito. Com `net` junto, o consentimento já
mostra "Rede" em âmbar.

### 3.3 `system` [feito: `cpu-profile`; resto aguarda]

Lista fechada de capacidades de sistema, cada uma uma operação que **o
launcher** executa com argumentos que ele mesmo monta; o módulo nunca escolhe
um comando, um caminho ou um argumento livre.

Fase 2: só `cpu-profile`. Efeito: `launch.before` pode devolver
`changes.governor` (um de `GOVERNORS`); o launcher acrescenta
`--governor=<g>` à linha do `runemu.sh` **só naquela execução**, sem tocar no
`system.cfg`. A biblioteca confere de novo o valor e só acrescenta em jogos
que abrem pelo `runemu.sh` (nunca em ferramentas ou ports).

**Distribuição:** ramo `feat/runemu-governor` (worktree
`distribution-wt/runemu-governor`), com teste
`projects/ArchR/packages/archr/tests/runemu_governor_test.sh`. Até ele entrar,
o `runemu.sh` ignora `--governor=*` (cai no `--*) continue`), o que é
inofensivo. O patch:

```sh
--governor=*) GOVERNOR_OVERRIDE="${ARG#*=}" ;;
...
CPU_GOVERNOR=$(get_setting "cpugovernor" "${PLATFORM}" "${ROMNAME##*/}")
case "${GOVERNOR_OVERRIDE}" in
  performance|ondemand|schedutil|powersave) CPU_GOVERNOR="${GOVERNOR_OVERRIDE}" ;;
esac
```

(a lista repetida no script é proposital: quem chama não é confiável).

Fase 3, proposta para aprovação (cada item com ameaça):

| capacidade | o que o launcher faz | ameaça | mitigação |
|---|---|---|---|
| `brightness` | `system.call {name:"brightness", value:0–100}` pelo mesmo caminho do OSD | tela apagada (golpe) | mínimo 10 %, 1 por segundo |
| `volume` | idem, pelo caminho do OSD | barulho | máximo = valor do usuário, 1 por segundo |
| `wifi-toggle` | `wifictl enable/disable` | cortar a internet do usuário | só com o launcher na tela, aviso na faixa |
| `screenshot` | captura da tela do launcher para `screenshots/` | vazar o que está na tela (Wi-Fi digitado) | nunca com teclado aberto; faixa "X capturou a tela" |

Nunca: desligar, reiniciar, suspender, atualizar, apagar, montar, mudar
senha, rodar script arbitrário.

### 3.4 `launch` e o pedido `game.launch` [feito]

```json
{"jsonrpc":"2.0","id":"l1","method":"game.launch",
 "params":{"system":"snes","game_key":"snes/zelda.sfc"}}
```

Regras, para não burlar o supervisor:

1. Só com `launch = true` concedido.
2. Só **até 5 s depois de o usuário escolher uma ação do próprio módulo**
   (bloco, atalho, item do Y, linha de ação) — e **uma vez** por escolha. Um
   módulo nunca abre um jogo sozinho, por timer ou por evento.
3. O launcher resolve `game_key` na biblioteca. Só jogos **emulados** (abrem
   pelo `runemu.sh`); ferramentas, ports e coleções (`@...`) são recusados.
   Ameaça que isso fecha: um módulo com gravação colocar um script em `ports/`
   e abri-lo como root.
4. O pedido vira `Outcome::Launch` **diferido**, pelo mesmo caminho da escolha
   do usuário: espera gravações pendentes, fecha Configurações, roda o
   `launch.before`, manda `game.started`, para todos os módulos (inclusive o
   que pediu), faz a transição e entrega a tela. Nada pula etapa.
5. Se houver lobby de netplay, Moonlight aberto ou outra abertura pendente, ou
   o sistema ainda não estiver carregado, o pedido é recusado (log) — a
   resposta `{}` só diz que o launcher recebeu.

### 3.5 `write` [aguarda teste no aparelho]

Aliases (nunca caminho livre):

| alias | pasta | por que só essas |
|---|---|---|
| `savestates` | `/storage/roms/savestates` | dados do usuário, não executáveis |
| `screenshots` | `/storage/roms/screenshots` | idem |

Fora: a raiz de `roms`, `bios`, `ports`, `tools`, qualquer pasta com conteúdo
que o launcher abre (escrever lá = executar como root no próximo jogo) e os
backups (continuam `InaccessiblePaths`).

**Saves** (`.srm`) ficam ao lado das ROMs (`savefiles_in_content_dir = true`
no `retroarch.cfg`), misturados com conteúdo abrível: não dá para conceder
gravação só neles. Proposta para a distribuição, a decidir: mover os saves
para `/storage/roms/saves/<sistema>` e então criar o alias `saves`. Até lá, um
módulo de backup de saves pede `read = ["roms"]` (lê) e não grava.

Sandbox: `BindPaths=-<pasta>` (em vez de `BindReadOnlyPaths`), e o
`InaccessiblePaths` dos backups continua por cima.

Dono dos arquivos: as pastas são do root. O launcher concede ao uid do módulo
uma **ACL POSIX** (`CONFIG_EXT4_FS_POSIX_ACL=y` no kernel) na pasta e em tudo
dentro dela, mais a ACL padrão (arquivos novos do RetroArch também ficam
graváveis para o módulo), escrita pelo próprio launcher (`setxattr`
`system.posix_acl_access`/`_default`; a imagem não tem `setfacl`). A
concessão é andando a árvore com `O_NOFOLLOW` (link nunca é seguido). Remover
o módulo ou a permissão tira as entradas.

Antes de liberar, testar no R36S: a ACL de um uid com `PrivateUsers=yes`
(o uid do módulo é mapeado, então deve valer), a montagem de `/storage` com
`acl`, e o tempo para percorrer `savestates` grande.

Ameaças: apagar ou corromper savestates; encher o cartão. Mitigações:
consentimento sempre em âmbar ("Grava e apaga seus savestates"); sem `net`, o
dano fica no aparelho (com `net`, o consentimento mostra os dois juntos);
`LimitFSIZE=64M` por arquivo; o launcher soma, a cada início do módulo, o
tamanho dos arquivos do uid dele nessas pastas e desliga o módulo acima de
512 MB, com aviso.

### 3.6 Eventos, tempos e taxas (tabela completa da API 2)

| mensagem | quem recebe | tempo limite | taxa |
|---|---|---|---|
| `launcher.started` | assinantes | — | 1 por sessão |
| `focus.system`, `focus.game` | daemons assinantes | — | após 400 ms parado; só quando muda |
| `game.started` | assinantes | — | 1 por jogo; o processo para em seguida (500 ms) |
| `game.ended` | assinantes | — | 1 por volta |
| `settings.changed` | o próprio daemon | — | 1 por mudança |
| `launch.before` (pedido) | daemons com `hooks:launch` | 300 ms, todos juntos; atraso = allow, 3 seguidos = 1 falha | 1 por jogo |
| `action.invoke` (pedido) | o módulo | 1 s daemon, 2 s oneshot | 1 por toque do usuário |
| `ui.update` | — | — | a tela junta: 4 por segundo |
| `ui.notify` | — | — | 1 a cada 10 s |
| `config.set` (pedido do módulo) | — | — | 1 por segundo, 10 por hora; valor igual não conta |
| `game.launch` (pedido do módulo) | — | — | 1 por ação do usuário, em 5 s |

A fila para cada daemon tem 64 mensagens; cheia = o módulo parou de ler =
falha.

## 4. Selo verificado e índice assinado

### 4.1 Hoje (fase 1 + este ramo) [feito]

`reviewed.json` (asset da última release de `archr-linux/archr-modules`):

```json
{"modules":[{"repo":"dono/repo","digest":"sha256:<hex>"}]}
```

- Na Loja, a oferta mostra o selo quando o digest do tarball da release
  (o SHA-256 que o GitHub publica para o asset, conferido de novo no download)
  está na lista para aquele repositório.
- **Novo:** o módulo **instalado** só tem selo se o digest do pacote que está
  no aparelho (`modules.toml`, `digest`) estiver na lista. Antes, bastava o
  repositório estar na lista e a versão ser igual (um autor podia publicar
  outro pacote com a mesma versão). A aba Instalados não mexe mais no selo
  (ela só lê o disco).
- **Novo:** na lista da Loja, o cartão só tem selo se o pacote (`.tar.gz`) da
  release **atual** tiver um digest da lista; um repositório revisado no
  passado com release nova não revisada sai da aba Revisados. A comparação ignora maiúsculas e o prefixo `sha256:`.

### 4.2 Índice assinado [aguarda aprovação do dono]

Ameaça que sobra: quem toma a conta `archr-linux` no GitHub publica um
`reviewed.json` com qualquer digest. TLS não ajuda (o servidor é o legítimo).

Proposta:

- `reviewed.json` v2:
  `{"version":2,"serial":42,"modules":[{"repo","id","version","digest"}]}`;
- `reviewed.json.sig`: assinatura Ed25519 (formato minisign) do arquivo
  inteiro, feita **fora do GitHub** (chave offline do mantenedor);
- o launcher embute 2 chaves públicas (atual + reserva, para rotação) e só
  aceita o índice com assinatura válida;
- `serial` só cresce: o launcher guarda o maior visto e recusa um índice
  menor (ataque de reverter para um índice antigo que ainda listava uma versão
  ruim);
- sem índice válido: nada é revisado (como hoje sem rede).

Custo: uma dependência Rust de Ed25519 (`ed25519-dalek`, sem C) e o dono gerar
o par de chaves. Revogar uma revisão = publicar índice sem a entrada; o selo
some na próxima consulta e a atualização automática para.

## 5. Atualização automática [aguarda aprovação: UI nova]

- Interruptor global em Configurações › Sistema › Módulos: **"Atualizar
  sozinho os revisados"**, desligado por padrão. Maquete
  `Modulos-auto.dc.html`.
- Só atualiza sozinho quando **todas** valem: a versão nova está no índice
  revisado (assinado, quando 4.2 existir) com o digest exato; ela não pede
  nenhuma permissão nova (`Permissions::without` vazio); a fonte é a mesma; há
  rede; o launcher está ocioso na tela (nunca durante jogo, nunca com
  Configurações abertas). Senão, fica "→ 1.2.0 disponível" como hoje.
- No máximo uma verificação a cada 24 h, 2 min depois do início.
- Mesmo caminho da instalação manual: digest, `module.toml` idêntico,
  validação, a versão anterior guardada até a nova responder; 3 falhas
  voltam a anterior sozinha (já existe).
- Aviso na faixa: "Perfil de CPU foi atualizado para 1.2.0".

Ameaça: a conta do autor (não a do ArchR) publica versão maliciosa → não
atualiza sozinha, porque o digest não está no índice. Conta do ArchR →
mitigado pela assinatura offline (4.2). Por isso a atualização automática
depende de 4.2 para sair do "aguarda".

## 6. Fase 3 (proposta)

Ordem sugerida por valor/risco:

1. **API de rede com domínios declarados.** `net = ["api.exemplo.org"]` em vez
   de `net = true`. O módulo roda com `PrivateNetwork=yes` e recebe só um
   socket `AF_UNIX` de um proxy do launcher (CONNECT HTTPS e HTTP simples) que
   deixa passar apenas os domínios declarados (sem IP literal, sem rede local;
   resolve o nome e recusa endereços privados, contra rebinding). O
   consentimento lista os domínios. `net = true` continua existindo, sempre em
   âmbar, e perde o selo por padrão (revisor justifica). Fecha o envio de ROMs e
   capturas para qualquer lugar.
2. **Widgets no Start.** `[[start.widgets]]` declarativo: bloco 2×1 com título,
   valor grande, linha secundária e barra opcional (0–100), atualizado por
   `ui.update` com `{value, detail, progress}`. Posição e tamanho vêm do
   `theme.toml` (o tema decide o layout, como os outros blocos); 1 por módulo,
   2 no total. Maquete `Start-widget.dc.html`.
3. **Página rica.** Tipos de linha novos, ainda declarativos: `progress`
   (barra), `list` (até 20 itens com título/subtítulo e ação por item, para
   listas dinâmicas vindas do módulo por `ui.update` com `items`), `image`
   (PNG do próprio `module-data`, até 256×192, decodificado pelo launcher com
   limite de bytes). Layout fixo de Configurações. Maquete
   `Pagina-rica.dc.html`.
4. **Ações de `system`** da tabela de 3.3, uma a uma.
5. **Saves** pelo alias `saves`, se a distribuição mover os `.srm`.
6. **Orçamento**: o launcher mostra em Detalhes a memória e o tempo de CPU de
   cada daemon (do cgroup do unit), para o usuário achar o módulo pesado.

## 7. Telas novas ou alteradas (maquetes, sem implementação)

Pasta: `/tmp/claude-1000/-media-disco-local-ArchR/4aaf2e6f-cc32-4024-87aa-c7ce177206ea/scratchpad/maquetes-modulos-fase2/`.

| arquivo | tela | fase |
|---|---|---|
| `Loja-modulo-fase2.dc.html` | permissões da API 2 na oferta (lista rola) | 2 |
| `Loja-modulo-atualizacao.dc.html` | atualização que pede permissão nova: só as novas | 2 |
| `Modulos-auto.dc.html` | Módulos com "Atualizar sozinho os revisados" | 2 |
| `Modulo-detalhes.dc.html` | detalhes: permissões concedidas, selo pelo digest, remover | 2 |
| `Start-widget.dc.html` | widget 2×1 no Start | 3 |
| `Pagina-rica.dc.html` | página com `progress` e `list` | 3 |

O que muda sem tela nova (já feito): linhas de permissão novas na lista
existente (mesmo componente). Hoje a coluna corta com "+N" quando não cabe; numa
tela de consentimento o usuário precisa ver tudo, então a coluna passa a rolar
(maquete `Loja-modulo-fase2.dc.html`) — aguarda aprovação junto com as telas.

## 8. Pendências

- Aprovação do dono: telas da seção 7; atualização automática; índice
  assinado (chave e dependência); ações de `system` da fase 3; proposta de
  mover os saves.
- Distribuição: merge de `feat/runemu-governor` (3.3).
- Teste no aparelho: `write` com ACL (3.5); DNS de um módulo com `net` e
  `IPAddressDeny=localhost`; `/run/dbus` e `/run/systemd/userdb` escondidos
  sem efeito colateral.
- A coluna de permissões da oferta corta com "+N" e precisa rolar (UI, com a
  maquete). Até lá, as sensíveis (âmbar) vêm primeiro na lista [feito], então um
  corte nunca esconde uma delas antes das outras.
