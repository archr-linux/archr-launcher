# Módulos do archr-launcher

Um **módulo** é um programa externo que o launcher inicia isolado e com quem
conversa por JSON. O módulo **declara** no `module.toml` o que acrescenta
(bloco no Start, item no menu Y, página em Configurações, eventos que quer
receber) e o launcher desenha tudo com os próprios componentes. O módulo nunca
desenha pixels, nunca lê o controle e nunca roda como root.

Este guia descreve a **API 2**. Módulos com `api_version = 1` continuam
rodando; as permissões `config_write`, `hooks`, `system` e `launch` exigem
`api_version = 2`. A especificação completa das fases 2 e 3 (ameaças,
mitigações, o que ainda aguarda) está em
`docs/superpowers/specs/2026-10-29-modulos-fases-2-3.md`. Linguagens garantidas na imagem:
**sh/bash + `jq`** e **binários aarch64**. Python pode existir na imagem, mas
um módulo não pode depender dele (`runtime = "python3"` é recusado).

Exemplos completos e funcionando: `examples/modules/bateria-detalhada`
(daemon) e `examples/modules/perfil-cpu` (oneshot).

## 1. Pacote

```
<id>/
  module.toml          obrigatório
  main.sh | bin/<binário>
  locales/en.toml      obrigatório; pt_BR.toml, es.toml, ru.toml recomendados
  icons/<nome>.svg     SVGs Lucide 0.468.0 sem modificação, só os usados
  preview.png          640×480, mostrado na Loja
  LICENSE
  README.md
```

A pasta tem o nome do `id`. Conferir à mão:

```
archr-launcher --check-module examples/modules/bateria-detalhada
```

Sai com código ≠ 0 e a lista de problemas; avisos (chaves desconhecidas,
tipos errados) aparecem mas não impedem.

## 2. Manifesto (`module.toml`)

```toml
id = "bateria-detalhada"        # [a-z0-9-]{3,40}, único no aparelho
version = "1.0.0"               # x.y.z
api_version = 2                 # 1 ou 2
min_launcher = "0.1.0"          # opcional
name = "@name"                  # texto, "@chave" do locales/, ou tabela por idioma
description = "@description"
author = "Fulano <fulano@exemplo.org>"
license = "MIT"
homepage = "https://github.com/fulano/archr-bateria"
icon = "battery-full"           # nome Lucide; o SVG vem em icons/

[run]
runtime = "bash"                # "sh" | "bash" | "native"
entry = "main.sh"               # caminho dentro do módulo
mode = "daemon"                 # "daemon" | "oneshot"

[limits]
memory_mb = 16                  # padrão 32, entre 8 e 64

[permissions]                   # ausente = negado
# net = true
# read = ["roms"]
# config_read = ["system.language"]
# config_write = ["system.cpugovernor"]   # API 2
# hooks = ["launch"]                      # API 2
# system = ["cpu-profile"]                # API 2
# launch = true                           # API 2

[[hooks]]
event = "focus.game"
```

**Erros fatais** (o módulo não roda): `id`, `version`, `api_version`, `[run]`
inválidos; permissão desconhecida ou ainda não disponível; mais de 48 linhas
de página; ícone que não é um SVG Lucide intacto; falta de `locales/en.toml`.
Todo o resto inválido vira aviso e o campo é ignorado.

`name` e `description` em tabela por idioma (`{ en = "...", pt_BR = "..." }`)
aparecem já na Loja, antes da instalação; com `@chave` a Loja mostra o nome do
repositório até o módulo estar instalado.

### 2.1 Pontos de extensão

**Bloco no Start** (até 2 por módulo, 4 no total; a fileira rola quando não cabe):

```toml
[[start.tiles]]
id = "bateria"
label = "@tile"
icon = "battery-full"           # opcional; padrão: o ícone do módulo
open = "main"                   # abre a página "main" em Configurações
# action = "atualizar"          # ou envia action.invoke
```

O bloco mostra um valor ao vivo embaixo do título quando o módulo manda
`ui.update` para `tile:<id>`.

**Atalho no Start** (`[[start.shortcuts]]`, mesmos campos; 1 por módulo, 2 no
total), entre Wi-Fi e Suspender. O rótulo aparece embaixo da fileira quando o
atalho tem o foco.

**Item no menu Y** (1 por módulo, 3 no total), antes de Suspender:

```toml
[[quick]]
id = "sugerir"
label = "@quick.suggest"
context = "game"                # "game" | "system"
systems = ["n64", "psp"]        # opcional; vazio = todos
action = "sugerir"
```

A ação recebe `context = {system, game_key, rom}` (jogo) ou `{system}`.

**Páginas em Configurações** (até 8 páginas, 48 linhas no total). Ficam em
Configurações › Sistema › Módulos › <módulo>. O layout é o de todas as páginas
de Configurações (fixo); o tema muda só cores, fontes e ícones.

```toml
[[settings.pages]]
id = "main"
title = "@title"

[[settings.pages.rows]]
type = "toggle"
key = "watts"
label = "@row.watts"
subtitle = "@row.watts_sub"     # opcional
section = "@section.options"    # opcional: cabeçalho acima da linha
default = true
```

| `type` | comportamento | campos |
|---|---|---|
| `toggle` | interruptor | `key`, `default` (bool) |
| `check` | caixa marcada com A | `key`, `default` (bool) |
| `value` | ‹ › entre escolhas | `key`, `choices`, `labels`, `default` |
| `slider` | ‹ › numérico (vira um `value` com os passos) | `key`, `min`, `max`, `step`, `default` |
| `action` | A envia `action.invoke`; mostra "…" até a resposta | `action`, `confirm` (pergunta opcional) |
| `text` | só leitura; valor vem de `ui.update` para `row:<id>` | `id`, `value` inicial |
| `keyboard` | abre o teclado do launcher | `key`, `max` (≤ 64), `secret` |

Os valores são do launcher (um dono por chave): ele grava
`/storage/.config/archr-launcher/module-settings/<id>.json` de forma atômica.
O módulo lê esse arquivo (`ARCHR_MODULE_SETTINGS`), nunca escreve nele; toggles e
checks viram booleanos, sliders viram números, o resto texto.

Linhas `keyboard` com `secret = true` ficam fora desse arquivo: vão para
`module-settings/<id>.secret.json` (`ARCHR_MODULE_SECRETS`), modo 0600, do
usuário do próprio módulo. O valor também chega no `initialize` e no
`settings.changed`, pelo stdin; nunca em argumento de processo nem no log.

### 2.2 Permissões

| permissão | efeito | texto ao usuário |
|---|---|---|
| (nenhuma) | só a própria pasta, `/sys` e `/proc` para leitura | "Sem acesso especial" |
| `net = true` | rede liberada (sem ela: `PrivateNetwork`, só `AF_UNIX`) | "Rede · Acessa a internet" (âmbar) |
| `read = ["roms", "bios", "screenshots"]` | `/storage/roms`, `/storage/roms/bios`, `/storage/roms/screenshots` só leitura | "Lê suas ROMs" … (âmbar junto com `net`) |
| `config_read = [...]` | `config.get` só das chaves listadas | "Lê ajustes" |
| `config_write = [...]` | `config.set` só das chaves listadas, valores conferidos (API 2) | "Muda ajustes" (âmbar) |
| `hooks = ["launch"]` | recebe `launch.before` antes de cada jogo (API 2) | "Vê cada jogo que você abre" |
| `system = ["cpu-profile"]` | em `launch.before`, pode escolher o governador daquela abertura (API 2) | "Escolhe o perfil de CPU de um jogo" |
| `launch = true` | `game.launch` logo após o usuário escolher uma ação do módulo (API 2) | "Abre jogos" |

As permissões em âmbar aparecem primeiro na lista.

Chaves aceitas em `config_read`: `system.language`, `system.timezone`,
`system.cpugovernor`, `system.hostname`, `global.netplay.nickname`,
`global.retroachievements`, `audio.volume`, `display.brightness`. Senhas,
tokens e chaves de Wi-Fi **nunca**.

Chaves aceitas em `config_write`, com os valores que passam:

| chave | valores |
|---|---|
| `system.cpugovernor` | `performance`, `ondemand`, `schedutil`, `powersave` (o `runemu.sh` executa o valor; nada mais passa) |
| `global.netplay.nickname` | `[A-Za-z0-9 _.]`, 1 a 32 caracteres |

Mesmo com `roms`, `/storage/roms/backup` (backups do `backuptool`, com o
`system.cfg` e suas senhas) e `/storage/roms/backups` (chaves de pareamento
Bluetooth) ficam inacessíveis (`InaccessiblePaths`).

Reservada (pedir hoje bloqueia o módulo): `write`, que espera o teste das ACLs
no aparelho.

O usuário vê as permissões antes de instalar ("Permitir e instalar"). Um
módulo copiado à mão aparece desligado com "Aguardando sua permissão" e a mesma
tela no primeiro uso. Se uma atualização pede permissões novas, a Loja mostra só
as novas e pede consentimento outra vez; sem consentimento para tudo o que pede,
o módulo não roda.

## 3. Execução

Cada processo roda com `systemd-run --pipe` como o **usuário do próprio
módulo**: um uid (igual ao gid) por id, de 60600 a 61099, dado na primeira vez
que o launcher vê o módulo e guardado em `modules.toml` (`[uids]`); remover o
módulo com os dados libera o uid. O launcher escreve o registro do usuário
(`archr-mod-<uid>`) em `/run/userdb`, que o nss-systemd lê, porque o systemd 255
recusa `User=` numérico sem usuário conhecido. Assim um módulo não lê o
`/proc`, o ambiente, os arquivos nem os sinais de outro: `ProtectProc=invisible`
esconde processos de outros uids, e `PrivateUsers=yes` mostra todo uid que não
seja root ou o dele como `nobody` (`PrivatePIDs=` só existe a partir do
systemd 257). Também: `NoNewPrivileges`, sem capabilities, `ProtectSystem=strict`,
`TemporaryFileSystem=/storage:ro` (toda a partição do usuário escondida),
de volta só a pasta do módulo (de root, só leitura), `module-data/<id>`
(gravável, é o `HOME`; 0700, do uid do módulo) e os arquivos de ajustes e de
segredos (só leitura); `/run/dbus` e os sockets do userdbd
(`/run/systemd/userdb`) inacessíveis (nenhum módulo fala com os serviços do
sistema); com `net`, só sockets `AF_UNIX`, `AF_INET` e `AF_INET6` e nada em
`localhost` (`IPAddressDeny=localhost`), exceto o stub de DNS do
systemd-resolved (`127.0.0.53`/`.54`) quando o `resolv.conf` aponta para ele;
um `resolv.conf` com `127.0.0.1` ou `::1` deixa o módulo sem DNS;
`MemoryMax`, `TasksMax=16`,
`CPUWeight=20`, `Nice=10`. Para ver a saída de erro no aparelho:
`journalctl -u 'archr-mod-<id>*'`.

| modo | vida | uso |
|---|---|---|
| `oneshot` | um processo por mensagem: 1 linha entra, termina | ações do usuário, eventos raros |
| `daemon` | um processo enquanto o launcher está na tela | valores ao vivo, eventos de foco |

- Iniciar um processo custa ~190 ms no R36S. Por isso eventos de foco só vão
  para **daemons**.
- Daemons sobem um por vez, depois do primeiro quadro, ~2 s após o início.
  No máximo **3 daemons** ligados; o quarto é recusado com aviso.
- **Nenhum módulo roda durante um jogo.** Antes de entregar a tela ao jogo o
  launcher manda `game.started`, depois `shutdown`, espera 500 ms e mata o
  que restou. Quando o launcher volta, o daemon recebe `game.ended`.
- Fim do stdin (EOF) = sair.

### Falhas

Contam como falha: tempo esgotado, JSON inválido, saída com erro ≠ 0, morrer
antes de responder, parar de ler o stdin. **3 falhas em 10 min desligam o
módulo** com o aviso "O módulo X foi desligado porque parou de responder"; o
usuário religa em Configurações › Módulos. Daemon que cai é reiniciado depois
de 1 s, 5 s e 30 s. Se a versão que falha acabou de ser atualizada pela Loja, a
versão anterior volta sozinha.

Tempos limite: `initialize` 3 s; `action.invoke` 1 s (daemon); um oneshot
inteiro 2 s.

## 4. Protocolo

JSON-RPC 2.0, um objeto por linha (UTF-8, `\n`, até 64 KiB) no stdin/stdout.
Stderr é log (as primeiras linhas vão ao log do launcher; o resto ao journal).
Toda mensagem tem `"jsonrpc": "2.0"`.

### Ambiente

`HOME` e `ARCHR_MODULE_DATA` (pasta gravável), `ARCHR_MODULE_DIR` (código, só
leitura), `ARCHR_MODULE_SETTINGS` (JSON dos ajustes), `ARCHR_MODULE_SECRETS`
(JSON das linhas `secret`; ausente sem elas), `ARCHR_MODULE_ID`,
`ARCHR_API_VERSION`, `ARCHR_LAUNCHER_VERSION`, `ARCHR_LANG`, `ARCHR_DEVICE`.

### Launcher → módulo, pedidos (têm `id`, exigem resposta)

| método | params | result |
|---|---|---|
| `initialize` (só daemon) | `{api_version, launcher_version, language, device, settings, permissions, capabilities}` | `{"ready": true}` |
| `action.invoke` | `{action, context: {system?, game_key?, rom?}}` | `{}` |
| `shutdown` (só daemon) | `{}` | `{}`, e sair |
| `launch.before` (só daemon, com `hooks = ["launch"]`) | `{game: {system, game_key, name}}` | `{"decision": "allow"}` ou `{"decision": "modify", "changes": {"governor": "<g>"}}` |

`launch.before`: todas as respostas juntas têm **300 ms**; quem atrasa vale como
`allow` naquela abertura (o jogo nunca espera); 3 atrasos seguidos contam uma
falha. Não há `deny`: um módulo não
impede o usuário de jogar. `changes.governor` só vale com
`system = ["cpu-profile"]` e um dos quatro governadores; o launcher passa
`--governor=<g>` ao `runemu.sh` só naquela execução, sem mudar o ajuste do
usuário. Se dois módulos escolhem, vale o primeiro em ordem de `id`. Netplay e
Moonlight não perguntam.

### Launcher → módulo, eventos (sem `id`)

`{"jsonrpc":"2.0","method":"event","params":{"name":"focus.game","data":{...}}}`,
só para quem assinou em `[[hooks]]`:

| evento | data | para |
|---|---|---|
| `launcher.started` | `{cold}` (`false` na volta de um jogo) | daemon e oneshot |
| `game.ended` | `{game:{system, game_key, name}, exit_code, seconds}` | daemon e oneshot |
| `game.started` | `{game:{system, game_key, name}}` | daemon e oneshot (o processo para logo depois) |
| `focus.system` | `{system}`, após 400 ms parado | só daemon |
| `focus.game` | `{system, game_key, name}`, após 400 ms parado | só daemon |
| `settings.changed` | `{key, value}` | daemon (sempre, sem assinar) |

### Módulo → launcher

| método | tipo | params |
|---|---|---|
| `ui.update` | notificação | `{target: "tile:<id>" \| "shortcut:<id>" \| "row:<id>" \| "quick:<id>", value?: texto ou {key, args}, hidden?: bool}` |
| `ui.notify` | notificação | `{text: texto ou {key,args}}` ou `{key, args}`; faixa de aviso, no máx. 1 a cada 10 s |
| `log` | notificação | `{level: "info" \| "error", message}` |
| `settings.set` | pedido | `{key, value}`, só chaves declaradas no manifesto → `{}` |
| `config.get` | pedido | `{key}`, só com `config_read` → `{value}` (`null` se ausente) |
| `config.set` | pedido | `{key, value}`, só com `config_write` e valor aceito → `{}`; valor igual ao atual não grava nem conta; 1 por segundo e 10 por hora (`-32003`) |
| `game.launch` | pedido | `{system, game_key}`, só com `launch` → `{}` |

`game.launch` só vale até **5 s depois de o usuário escolher uma ação do
módulo** (bloco, atalho, item do Y, linha de ação), uma vez por escolha, e só
para jogos emulados (nunca ferramentas, ports ou coleções). O launcher abre o
jogo pelo mesmo caminho da escolha do usuário: espera gravações pendentes,
fecha Configurações, roda o `launch.before`, para todos os módulos e entrega a
tela. Se não puder (lobby aberto, outra abertura pendente, sistema ainda não
carregado), recusa e registra no log; o `{}` só confirma o recebimento.

Textos: `"@chave"` resolve no catálogo do módulo (idioma do launcher, depois
`en`); `{"key":"@st.watts","args":{"pct":87,"w":"1.9"}}` troca `{pct}` e `{w}`.
`ui.update` chega à tela no máximo 4 vezes por segundo (o launcher junta).
`hidden: true` esconde um bloco, atalho ou item até um `hidden: false`.

No **oneshot** o launcher escreve **uma** mensagem e fecha o stdin; o módulo
pode emitir notificações (`ui.update`, `ui.notify`, `log`) e, se havia `id`,
exatamente uma resposta. Não há `initialize` nem `shutdown`; os ajustes vêm de
`ARCHR_MODULE_SETTINGS`. Pedidos do módulo (`config.get`, `config.set`,
`settings.set`, `game.launch`) só funcionam em daemons.

### Erros

`error: {code, message}`: `-32700` JSON inválido, `-32600` pedido inválido,
`-32601` método desconhecido, `-32602` parâmetros inválidos, `-32001` permissão
negada, `-32002` tempo esgotado, `-32003` limite de taxa, `-32004` não
encontrado, `-32000` o launcher não conseguiu (ex.: gravar). Responda `-32601` a
métodos que não conhece (não conta como falha).

## 5. Exemplo 1 — "Bateria detalhada" (bash + jq, daemon)

Bloco no Start com porcentagem e potência; página com saúde da bateria, um
interruptor e o intervalo. Nenhuma permissão: só lê `/sys`. Arquivos completos
em `examples/modules/bateria-detalhada/`.

`module.toml` (trecho):

```toml
[run]
runtime = "bash"
entry = "main.sh"
mode = "daemon"

[[start.tiles]]
id = "bateria"
label = "@tile"
icon = "battery-full"
open = "main"

[[settings.pages]]
id = "main"
title = "@title"

[[settings.pages.rows]]
type = "text"
id = "agora"
label = "@row.now"
section = "@section.state"

[[settings.pages.rows]]
type = "toggle"
key = "watts"
label = "@row.watts"
section = "@section.options"
default = true

[[settings.pages.rows]]
type = "value"
key = "intervalo"
label = "@row.interval"
choices = ["5", "15", "30"]
labels = ["@every.5", "@every.15", "@every.30"]
default = "15"
```

`main.sh` (o essencial):

```bash
#!/bin/bash
send() { printf '%s\n' "$1"; }
reply() { send "{\"jsonrpc\":\"2.0\",\"id\":$1,\"result\":$2}"; }
load() {
    watts=$(jq -r 'if .watts == false then "false" else "true" end' "$ARCHR_MODULE_SETTINGS" 2>/dev/null)
    interval=$(jq -r '.intervalo // "15"' "$ARCHR_MODULE_SETTINGS" 2>/dev/null)
    watts=${watts:-true}; interval=${interval:-15}
}
update() { ...lê /sys/class/power_supply e envia ui.update para tile:bateria, row:agora, row:saude... }
handle() {
    eval "$(printf '%s' "$1" | jq -r '@sh "id=\(.id // "null") method=\(.method // "") name=\(.params.name // "")"')"
    case "$method" in
        initialize) load; reply "$id" '{"ready":true}'; update ;;
        event) [ "$name" = settings.changed ] && { load; update; } ;;
        action.invoke) update; reply "$id" '{}' ;;
        shutdown) reply "$id" '{}'; exit 0 ;;
        *) [ "$id" = null ] || send "{\"jsonrpc\":\"2.0\",\"id\":$id,\"error\":{\"code\":-32601,\"message\":\"$method\"}}" ;;
    esac
}
while true; do
    if IFS= read -r -t "$interval" line; then handle "$line"
    elif [ $? -gt 128 ]; then update      # passou o intervalo
    else exit 0; fi                        # EOF: o launcher saiu
done
```

Sem bateria (num PC) o módulo esconde o bloco com `hidden: true`.

## 6. Exemplo 2 — "Perfil de CPU" (sh + jq, oneshot)

Na fase 1 o módulo **sugere** o governador de CPU do sistema do jogo pelo
menu Y e mostra o governador atual numa página. Arquivos em
`examples/modules/perfil-cpu/`.

```toml
[run]
runtime = "sh"
entry = "main.sh"
mode = "oneshot"

[[quick]]
id = "sugerir"
label = "@quick.suggest"
context = "game"
action = "sugerir"
```

```sh
#!/bin/sh
IFS= read -r msg || exit 0
eval "$(printf '%s' "$msg" | jq -r '@sh "id=\(.id // "null") method=\(.method // "") action=\(.params.action // "") system=\(.params.context.system // "")"')"
setting() {
    value=$(jq -r --arg k "$1" '.[$k] // empty' "$ARCHR_MODULE_SETTINGS" 2>/dev/null)
    printf '%s' "${value:-$2}"
}
case "$system" in
    n64|psp|dreamcast|saturn|nds|psx) governor=$(setting pesados performance) ;;
    nes|gb|gbc|gamegear|mastersystem|atari2600|pcengine) governor=$(setting leves powersave) ;;
    *) governor=schedutil ;;
esac
jq -cn --arg s "$system" --arg g "$governor" \
  '{jsonrpc:"2.0",method:"ui.notify",params:{key:("@suggest."+$g),args:{system:$s}}}'
printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id"
```

**Com a API 2:** o mesmo módulo como daemon, com `api_version = 2`,
`hooks = ["launch"]` e `system = ["cpu-profile"]`, responde ao
`launch.before` com `{"decision":"modify","changes":{"governor":"performance"}}`
e o launcher passa `--governor=performance` ao `runemu.sh` só naquela
execução, sem mudar o ajuste do usuário; o item no menu Y deixa de ser
necessário. (O `runemu.sh` da imagem ainda ignora `--governor=`; a mudança na
distribuição está na especificação das fases 2 e 3.)

## 7. Testar

- À mão, um oneshot:
  `echo '{"jsonrpc":"2.0","id":1,"method":"action.invoke","params":{"action":"sugerir","context":{"system":"n64"}}}' | ARCHR_MODULE_SETTINGS=/dev/null sh main.sh`
- À mão, um daemon: mande `initialize`, depois o que quiser, e feche o stdin.
- No PC com uma raiz de testes (`ARCHR_LAUNCHER_ROOT=<pasta>`), o launcher roda
  os módulos **sem sandbox** e avisa no log: só para desenvolvimento.
- No aparelho: copie a pasta para `/storage/.config/archr-launcher/modules/<id>/`
  por SSH e ligue em Configurações › Sistema › Módulos (aparece como "Local").

## 8. Publicar na Loja de módulos

A Loja lista todo repositório público, não arquivado, com o tópico
**`archr-launcher-module`**. Os revisados pelo projeto ganham o selo
**Revisado pelo ArchR** e vêm primeiro: a lista é o `reviewed.json` da última
release de `archr-linux/archr-modules`, com repositório e digest do pacote
revisado; publicar outro pacote tira o selo até nova revisão. Um módulo
instalado só mostra o selo se o digest do pacote que está no aparelho estiver
na lista.

Regras da release (a última, não pre-release):

- tag `v<version>`, igual ao `version` do manifesto;
- assets: `<id>-<version>.tar.gz`, `module.toml` (cópia avulsa, idêntica byte a
  byte à do pacote) e `preview.png`;
- tarball até 8 MB, até 16 MB descompactado, até 500 entradas, tudo dentro de
  `<id>/`; só arquivos e pastas: link, dispositivo, caminho absoluto ou `..`
  recusam o pacote inteiro. Modos são normalizados (0644; a entrada, 0755).

```
tar -czf bateria-detalhada-1.0.0.tar.gz bateria-detalhada/
```

A Loja confere o SHA-256 que o GitHub publica para o asset, exige o
`module.toml` idêntico e valida o pacote antes de colocá-lo em `modules/<id>`.
Um `id` pertence a um repositório: outro repositório com o mesmo `id` é
recusado. Atualizações nunca são automáticas; a versão anterior fica guardada
até a nova funcionar.
