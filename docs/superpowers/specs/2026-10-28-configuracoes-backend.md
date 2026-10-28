# Configurações do launcher: referência de backend (o que o EmulationStation faz hoje)

Data: 2026-10-28. Escopo: lista P0 aprovada (12 itens). Documento de referência, somente leitura do estado atual.

Fontes lidas:

- ES como compilado (`-DARCHR=1`, `-DRK3326` via `DEVICE`, sem BATOCERA, sem Kodi):
  `distribution/build.ArchR-RK3326.aarch64/build/emulationstation-f25c4475a348f534de8ff724e9e42808b7a80a12/`.
  Abreviado aqui como `ES/`. Números de linha referem-se a essa árvore (já com os 5 patches de `packages/ui/emulationstation/patches`).
- Scripts e cola: `distribution/projects/ArchR/packages/` (abreviado `PKG/`).
- Imagem construída (para amostras reais de arquivos): `distribution/build.ArchR-RK3326.aarch64/image/system/` (abreviado `IMG/`).

Convenção de confiança: "Verificado" = lido no código/arquivo. "Inferido" = deduzido do código, não executado em aparelho.
Nenhum comando foi executado em um R36S nesta pesquisa; as amostras de saída marcadas "real" vêm de arquivos da imagem
(`zone1970.tab`, `base.lst`, `es_input.cfg`), as demais são o formato exato emitido pelo código.

---

## 0. Mecânica geral (vale para todos os itens)

### 0.1 O arquivo `system.cfg`

| Aspecto | Valor |
|---|---|
| Caminho | `/storage/.config/system/configs/system.cfg` (`ES/es-core/src/Paths.cpp:94`). Semente: `/usr/config/system/configs/system.cfg` (`PKG/archr/config/system/configs/system.cfg`), copiada por `set_setting` se faltar. |
| Formato | `chave=valor`, uma por linha, sem aspas, sem seções. Linhas começando por `#` ou `;` são ignoradas pelo ES. Valor vazio = chave ausente. |
| Chave por sistema | `<sistema>.<opção>` (ex.: `n64.video_plugin`). |
| Chave por jogo | `<sistema>["<nome-do-arquivo-com-extensão>"].<opção>`; no nome do arquivo o ES remove `=` e `#` (`ES/es-app/src/FileData.cpp:117`). |
| Chave global | `global.<opção>`. |
| Precedência na leitura pelos scripts | `get_setting <chave> <sistema> <jogo>` (`PKG/archr/profile.d/001-functions:28`): jogo, depois `<sistema>.<chave>`, depois `system.<chave>`, depois `<chave>` pura, depois `global.<chave>`. O ES na UI usa a mesma ideia para mostrar o "valor herdado" (`GuiMenu.cpp:3123-3145`). |

### 0.2 Como o ES lê e grava (importante para reproduzir)

| Ponto | Comportamento verificado | Fonte |
|---|---|---|
| Leitura | Lê o arquivo inteiro na inicialização (`loadSystemConf`) e mantém em memória; ES **não relê** o arquivo se um script o alterar depois. | `ES/es-core/src/SystemConf.cpp` |
| Gravação adiada | `SystemConf::set` só altera memória e marca a chave como "alterada". O arquivo só é reescrito em `saveSystemConf()`, chamado ao **fechar** a tela de menu (`GuiSettings::save`, só se a tela registrou alguma função de salvar) ou explicitamente. | `SystemConf.cpp:103,228`; `GuiSettings.cpp:82-95` |
| Remoção de chave | Ao gravar, valor vazio, `auto` ou `default` **removem a linha** (exceções: `audio.device`, `updates.branch`). Valor igual ao default interno (`kodi.enabled=1`, `wifi.enabled=0`, `system.hostname=BATOCERA`, ...) também remove. | `SystemConf.cpp` (tabela `defaults`, `dontRemoveValue`) |
| Sem trava | A reescrita copia um `.tmp` por cima do arquivo, **sem** usar a trava `/tmp/.system.cfg.lock` que os scripts usam (`wait_lock` em `001-functions`). Corrida possível com `set_setting` de scripts. | `SystemConf.cpp` fim de `saveSystemConf` |
| Escrita pelos scripts | `set_setting k v` = `del_setting k` + `echo k=v >>` sob trava; `v=default` apaga e não regrava. `del_setting` usa `sed "/^k=/d"`, frágil com `[`, `]`, `(`, `)` em chaves por jogo. | `001-functions:127-148, 107-115` |

Consequência para o launcher: **grave a chave no arquivo antes de chamar qualquer script que a leia com `get_setting`**.
O ES chama vários scripts com o valor ainda só na memória (ver armadilhas em cada item: automount, merged storage). Recomenda-se
escrever com a mesma trava (`noclobber` em `/tmp/.system.cfg.lock`) e gravação atômica (temp + `rename`).

### 0.3 Como o ES executa comandos

| Função | Comportamento | Fonte |
|---|---|---|
| `ApiSystem::executeScript(cmd)` | `system(cmd)`: **síncrono**, bloqueia a thread que chama; sucesso = exit code 0. | `ES/es-app/src/ApiSystem.cpp:1905` |
| `executeScript(cmd, func)` | `popen`, chama `func(linha)` para cada linha da saída; devolve `(última linha, exit code)`. Usado para streaming (Bluetooth, update). | `ApiSystem.cpp:1881` |
| `executeEnumerationScript(cmd)` | `popen`, devolve vetor de linhas. Síncrono. | `ApiSystem.cpp:1820` |
| `Utils::Platform::runSystemCommand(cmd,...)` | `fork` + `sh -c`, **espera terminar**, mas o processo intermediário faz `_exit(0)`: o resultado é **sempre 0**. O código de saída do script se perde (o patch 0005 do device-switch testa `!= 0`, portanto nunca dispara). | `ES/es-core/src/utils/Platform.cpp:724-751` |
| `Utils::Platform::GetShOutput(cmd)` | `popen`, concatena a saída **removendo todos os `\n`** (por isso listas multilinha viram texto contíguo). | `Platform.cpp:762` |
| `Utils::Platform::GetEnv("DEVICE_*")` | Variáveis de ambiente exportadas. | ver 0.4 |

Regra para o launcher: usar `std::process::Command` com argv (nunca `sh -c` com concatenação), ler o `status()` de verdade,
e rodar tudo que não for instantâneo em thread/tarefa assíncrona (marcado **ASYNC** nas tabelas).

### 0.4 Detecção por dispositivo (variáveis `DEVICE_*`)

O ES não sonda hardware; lê variáveis de ambiente. Elas vêm de `/etc/profile` + `/storage/.config/profile.d/*`
(escritos pelos quirks em `PKG/hardware/quirks/{platforms,devices}/...` no boot) e o serviço injeta o conjunto exportado
(`/etc/profile.d/999-export`, lista de nomes) com `systemctl import-environment` em `PKG/ui/emulationstation/sources/es_settings`
(que roda como `ExecStartPre` do `emustation.service`). O launcher precisa receber o mesmo ambiente (`EnvironmentFile=/etc/profile`
no serviço, ou `sh -lc 'echo $DEVICE_X'`).

| Variável | Quem define | Efeito no menu |
|---|---|---|
| `DEVICE_DTB_SWITCH=true` | `PKG/hardware/quirks/autostart/002-device-switch` (existe `/sys/firmware/devicetree/base/archr,device_switch/this`) | Mostra "THIS DEVICE IS" |
| `DEVICE_TURBO_MODE=true` | `platforms/RK3326/002-turbo-mode_config` | Mostra "ENABLE CPU OVERCLOCK" |
| `DEVICE_MMC_EJECT=false` | `devices/{Anbernic RG351M, Gameforce CHI, ODROID-GO Advance, ODROID-GO Advance Black Edition, ODROID-GO Super}/001-device_config` | **Oculta** bloco STORAGE inteiro (automount, merged, eject). Ausente = mostra. |
| `DEVICE_HAS_FAN=true` | Nenhum quirk de RK3326 define | "COOLING PROFILE" nunca aparece no RK3326 |
| `DEVICE_GPU_OVERCLOCK` | Nenhum quirk de RK3326 define | Switch de GPU overclock nunca aparece |
| `DEVICE_PWR_LED_CONTROL=true` | R33S, R36S, R36S Soysauce, R36S Clone | Não é P0 |
| `DEVICE_MANGOHUD_SUPPORT` | `platforms/RK3326/075-mangohud-supported` | Não é P0 |
| Diretivas de compilação | `-DRK3326` (CMake lê `ENV{DEVICE}`, `ES/CMakeLists.txt:98-100`) | Ativa o ramo "SUSPEND MODE" por `/sys/power/state`, "MULTIMEDIA KEYS", "BUTTON LED COLOR", "POWER LED COLOR" (`GuiMenu.cpp:2107, 2439, 2455, 2473`) |

### 0.5 Processos que o launcher precisa substituir (além das telas)

| Fato | Detalhe | Fonte |
|---|---|---|
| `es_settings` é pré-requisito do ES | `ExecStartPre=/usr/bin/es_settings` grava `/storage/.cache/timezone` e `/storage/.cache/system_timezone` a partir de `system.timezone`, reinicia `tz-data.service` se mudou, gera o locale (`localedef`, pode levar "um minuto ou dois" no RK3326 na primeira vez) a partir de `system.language`, e importa as `DEVICE_*`. Se o serviço do launcher não fizer o mesmo, fuso e idioma não se aplicam no boot. | `PKG/ui/emulationstation/sources/es_settings`; `system.d/emustation.service` |
| API HTTP do ES na porta 1234 | `archr-fake-suspend` chama `curl http://localhost:1234/runningGame` (200 = jogo rodando, 201 = nenhum) e `curl http://localhost:1234/shutdown` (desligamento gracioso). Se o launcher não expuser algo equivalente, o fake-suspend desliga com `shutdown now` sem salvar nada e trata "jogo rodando" como erro. | `PKG/archr/sources/scripts/archr-fake-suspend` (`check_es_running_game`, `do_shutdown`) |
| Eventos de scripts | O ES dispara `config-changed`, `controls-changed`, `quit`, `reboot`, `shutdown` para scripts em `/storage/.config/emulationstation/scripts/<evento>/`, `/run/emulationstation/scripts/<evento>/`. A imagem não traz nenhum. Nada depende disso hoje. | `ES/es-core/src/Scripting.cpp:157-190` |

---

## 1. Wi-Fi

Backend: `/usr/bin/wifictl` (bash, `PKG/archr/sources/scripts/wifictl`) sobre **iwd** (`iwctl`). Perfis em `/var/lib/iwd` (symlink para `/storage/.cache/iwd`).
Dispositivo: primeiro `/sys/class/net/wl*`. **Se não houver interface Wi-Fi, todo comando sai com 0 sem fazer nada** (`wifictl:25-26`): não dá para distinguir "ok" de "sem hardware" só pelo exit code.
Disponibilidade na UI do ES: `isScriptingSupported(WIFI)` = existe o executável `wifictl` (`ApiSystem.cpp:1932`); aparece em todos os aparelhos.

| Operação | Comando / chave | Saída e retorno | Bloqueia? | Reinício? | Fonte ES |
|---|---|---|---|---|---|
| Estado atual (ligado/desligado) | chave `wifi.enabled` (`1`/`0`; seed `0`; o pacote pode trocar para `1`, `PKG/archr/package.mk:51,195`). Não há leitura de estado real; ES confia na chave. Estado real (inferido): `rfkill list wifi`. | | não | não | `GuiMenu.cpp:5172,5227` |
| Ligar | `wifictl enable` (= `rfkill unblock wifi` + `set_usb_wifi_mux 0`) e depois conectar (abaixo). Gravar `wifi.enabled=1`. | exit 0 | rápido (< 1 s) | não | `ApiSystem.cpp:456-461` |
| Desligar | `wifictl disable` (= `rfkill block wifi` + mux USB para porta externa em placas com o mux) e gravar `wifi.enabled=0`. | exit 0 | rápido | não | `ApiSystem.cpp:492` |
| Estado da conexão | O ES não lê. Usar (inferido): `iwctl station <dev> show` (`State connected`, `Connected network <ssid>`); IP via `getifaddrs`. ES mostra só IP (`queryIPAddress`) e "INTERNET STATUS" por `ping -c 1 -W 2 8.8.8.8` (cai para 1.1.1.1, 9.9.9.9). | | `ping` até 6 s no pior caso: ASYNC | | `GuiMenu.cpp:5176-5201`; `ApiSystem.cpp:330` |
| Listar redes salvas/visíveis (sem varrer) | `wifictl list` | Um **SSID por linha**, ordenado por sinal (D-Bus `GetOrderedNetworks`). **Somente nomes**: sem segurança, sem sinal, sem marca de rede conhecida. Linhas vazias/ocultas são descartadas. Saída vazia se iwd não responde (o script engole exceções). | espera `wait_for_wifi` até 15 s se o iwd não estiver pronto | | `ApiSystem.cpp:1692`; `PKG/network/iwd/scripts/iwd_get-networks` |
| Varrer | `wifictl scanlist` (= `iwctl station <dev> scan`; `sleep 2`; lista) | Mesmo formato de `list`. Amostra: `MinhaCasa\nAndroidAP_1234\nVIVO-5G\n` | **~3 a 5 s, ASYNC** (o ES mostra spinner "SEARCHING WI-FI NETWORKS") | | `GuiWifi.cpp:102-117` |
| Só disparar varredura | `wifictl scan` | sem saída | rápido | | |
| Conectar WPA/WPA2/WPA3 (PSK) | `wifictl connect '<ssid>' '<senha>' '<país>'` (argv 2, 3, 4; omitidos = `wifi.ssid`, `wifi.key`, `wifi.country` do cfg). Senha vazia = rede aberta. | exit 0 conectado (confirma `State connected` **e** `Connected network == ssid`), 1 falhou/timeout, 2 senha inválida (PSK precisa de 8 a 63 bytes ou 64 hex; mensagem em stderr). Log detalhado em `/var/log/wifictl.log`. Não mostra o motivo da falha ao chamador. | **até ~60 s, ASYNC obrigatório** (espera iwd até 15 s + até 6 varreduras de 2 s + espera de associação 30 s). ES usa `GuiLoading` "CONNECTING TO WI-FI". | não | `ApiSystem.cpp:456-477`; `GuiMenu.cpp:5340-5370, 5398-5420` |
| WPA3 (SAE) | Sem tratamento específico: o script grava perfil `.psk` (`[Security] Passphrase=`); o iwd escolhe SAE ou PSK conforme o AP anuncia. Não há detecção de tipo na listagem. | idem | idem | | `wifictl:150-163` |
| Rede oculta | Automático: se o SSID não aparece em ~12 s de varredura, `wifictl` apaga o perfil, tenta `iwctl --passphrase ... connect-hidden`, e se falhar regrava o perfil e tenta o connect normal. Pelo ES: botão "INPUT MANUALLY" (digitar SSID) e depois a senha. **Não há marcação de "oculta" nem teste positivo registrado** (pendência conhecida). | idem | até ~60 s | | `wifictl:233-267`; `GuiWifi.cpp:57-64` |
| 802.1X (WPA-Enterprise) | **Sem UI nem CLI.** O `wifictl` só respeita um perfil `.8021x` já existente: se existir `/var/lib/iwd/<nome>.8021x`, mantém e não grava `.psk`/`.open`. O usuário precisa criar o arquivo à mão. Nome do arquivo: SSID cru se casa `^[A-Za-z0-9_ -]+$`, senão `=` + hexadecimal minúsculo de todos os bytes do SSID. Conteúdo iwd (ex. PEAP): `[Security]` `EAP-Method=PEAP`, `EAP-Identity=`, `EAP-PEAP-Phase2-Method=MSCHAPV2`, `EAP-PEAP-Phase2-Identity=`, `EAP-PEAP-Phase2-Password=`, opcional `EAP-PEAP-CACert=`. Depois `wifictl connect '<ssid>'` com senha vazia. O comentário no script cita `/storage/.cache/iwd/` (mesmo diretório, via symlink). | | | | `wifictl:119-138` |
| Esquecer rede | **Não implementado no ES nem no wifictl.** Implementação possível (inferido, validar): `iwctl known-networks '<ssid>' forget`, ou apagar `/var/lib/iwd/<nome codificado>.{psk,open,8021x}` e, se for a rede de `wifi.ssid`, limpar `wifi.ssid`/`wifi.key`. | | rápido | | n/a |
| País regulatório | chave `wifi.country` (código ISO 3166-1 alfa-2; vazio = N/A). Aplicado por `iw reg set` dentro do `wifictl` a cada conexão; se vazio, aprende dos IEs dos APs vizinhos só para a sessão. Lista de países hardcoded em `GuiMenu.cpp:5131+` (`getCountryCodes`). | | | | `GuiMenu.cpp:5267-5277` |
| Persistir para reconexão no boot | O boot (`PKG/archr/autostart/099-networkservices:51-54`) roda `wifictl enable` + `nohup wifictl connect` **sem argumentos** se `wifi.enabled=1`. Logo o launcher deve gravar `wifi.ssid` e `wifi.key` (texto puro no cfg, como o ES faz) da última rede usada, senão o boot tenta a rede antiga. O iwd também reconecta sozinho a redes conhecidas (inferido). | | | | `GuiMenu.cpp:5258-5262` |
| Modo AP/"local play" | `wifi.adhoc.enabled`, `wifi.adhoc.id` (1 a 4), `wifi.adhoc.channel` (lista de `wifictl channels`), `wifictl has_ap_mode` (exit 0 = suporta). Quando `wifi.adhoc.enabled=1` o `wifictl` ignora o SSID pedido e usa `NETPLAY_AP`/`deadbeef`. **Fora do P0, mas é armadilha:** se a chave ficar em 1, `connect` nunca conecta a rede normal. | | | | `GuiMenu.cpp:5268-5330` |

Amostra de `wifictl list` (formato derivado de `iwd_get-networks`, `print(ssid)` por rede):

```
MinhaCasa
VIVO-5G
AndroidAP_1234
```

---

## 2. Bluetooth

Backend: `/usr/bin/archr-bluetooth` (sh, `PKG/archr/sources/scripts/archr-bluetooth`) + daemon `archr-bluetooth-agent` (python, serviço `bluetooth-agent`) + `bluetoothd`.
Disponibilidade na UI: existe o executável `archr-bluetooth` (`ApiSystem.cpp:1935`); aparece mesmo em aparelhos sem rádio Bluetooth (o R36S depende de dongle). Não há teste de hardware.
Telas: `GuiControllersSettings.cpp:130-201`, `GuiBluetoothPair.cpp`, `GuiBluetoothDevices.cpp`, `GuiBluetoothDeviceOptions.cpp`.

**Defeito transversal: `archr-bluetooth` termina sempre com exit 0** (o `case` final cai em `exit 0`, os `return 1` das funções são descartados). O ES usa `executeScript(...)` que só olha o exit code; portanto "parear", "conectar", "esquecer" nunca falham do ponto de vista do ES. O launcher deve avaliar o resultado pela **saída** (`INFO:`/`ERROR:` e `/run/bt_status`) ou consultando `bluetoothctl info <MAC>`.

| Operação | Comando / chave | Saída e retorno | Bloqueia? | Fonte ES |
|---|---|---|---|---|
| Estado | chave `controllers.bluetooth.enabled` (`1`/`0`). Real (inferido): `systemctl is-active bluetooth bluetooth-agent`. Arquivo `/run/bt_agent_status` contém o PID do agente vivo. | | não | `GuiControllersSettings.cpp:128-138` |
| Ligar | Gravar a chave **e** `archr-bluetooth enable` (seta a chave com `set_setting`, `systemctl start bluetooth bluetooth-agent`, espera o agente até 5 s). | `INFO: Bluetooth agent started successfully` ou `ERROR: Failed to start bluetooth agent` (exit 0 nos dois). O agente ainda dorme 5 s antes de achar o adaptador. | **ASYNC** (5 a 10 s) | `ApiSystem.cpp:523` |
| Desligar | `archr-bluetooth disable` | `INFO: Bluetooth disabled` | ASYNC (alguns s) | `ApiSystem.cpp:528` |
| Varrer (listagem ao vivo) | `archr-bluetooth live_devices` (alias `start_live_devices`). **Processo de longa duração:** escreve `start` em `/run/bt_discovery_control`, e fica em `tail -f /run/bt_listing` até receber sinal. Cada evento é uma linha XML. Parar: `archr-bluetooth stop_live_devices` (mata o `tail`, escreve `stop`). | `<device id="AA:BB:CC:DD:EE:FF" name="8BitDo SN30 Pro" status="added" type="joystick" />` (`status` = `added`/`removed`; `type` = `joystick` (Icon `input-gaming`), `audio` (Icon `audio-*`), outro Icon BlueZ cru ou `unknown`). Se o agente não roda: `ERROR: Bluetooth agent is not running` e sai. | **Streaming ASYNC**, lido linha a linha | `ApiSystem.cpp:533-541`; `GuiBluetoothPair.cpp:70-110` |
| Parear (dispositivo escolhido) | `archr-bluetooth trust <MAC>` (MAC em **maiúsculas**, regex `^([0-9A-F]{2}:){5}[0-9A-F]{2}$`). Escreve o MAC em `/run/bt_device`; o agente pareia (`Pair`), marca `Trusted` e conecta (até 5 tentativas). O script espera até 60 s imprimindo cada mudança de `/run/bt_status`, e sai quando o MAC aparece em `list` (Trusted=true). Ao fim, `save` (tar em `/storage/roms/backups/bluetooth.tar`). **Requer que a varredura (live_devices) esteja ativa**, porque o agente só vê o dispositivo se o adaptador estiver em discovery. | Mensagens: `Pairing <nome>...`, `Trusting <nome>...`, `Connecting <nome>...`, `Connected successfully (<nome>)`, `Connection failed. Giving up. (<nome>)`, `Pairing failed (<nome>)`. exit 0 sempre. | **até 60 s, ASYNC** | `ApiSystem.cpp:543`; `GuiBluetoothPair.cpp:126-141` |
| Parear controles automaticamente | `archr-bluetooth trust input`: aceita qualquer dispositivo com Icon `input*`, até 60 s, repassa as mensagens de status. **Provável defeito:** o script não escreve `start` no arquivo de controle (só `live_devices` faz), então o agente não inicia discovery e a opção só funciona se alguém já estiver varrendo. Verificar no aparelho antes de reutilizar. | linhas de status como acima | até 60 s, ASYNC | `ApiSystem.cpp:563`; `ThreadedBluetooth.cpp` |
| Conectar (já pareado) | `archr-bluetooth connect <MAC>` (usa `bluetoothctl -- connect`) | `INFO: Device X is already connected` / `INFO: Successfully connected to device X` / `ERROR: Failed to connect to device X` | segundos, ASYNC | `ApiSystem.cpp:548` |
| Desconectar | `archr-bluetooth disconnect <MAC>` | `INFO: Successfully disconnected device X` / `ERROR: Device X is not connected` | ASYNC | `ApiSystem.cpp:553` |
| Listar pareados | `archr-bluetooth list` (lê `/storage/.cache/bluetooth/*/*/info` com `Trusted=true`, consulta `bluetoothctl info` por dispositivo) | `<device id="AA:BB:CC:DD:EE:FF" name="8BitDo SN30 Pro" type="joystick" connected="yes" />` uma por linha. `type` só reconhece classes `0x002508`/`0x000508` como `joystick`; fones aparecem com `type=""`. Mais lento com muitos dispositivos (um `bluetoothctl` por item). | ASYNC | `ApiSystem.cpp:568` |
| Esquecer um | `archr-bluetooth remove <MAC>` (untrust + remove no bluetoothctl, apaga a pasta em `/storage/.cache/bluetooth`, refaz o backup tar) | `INFO: Device X removed successfully` | ASYNC | `ApiSystem.cpp:558`; `GuiBluetoothDevices.cpp:125+` |
| Esquecer todos | `archr-config forgetBT` (para `bluetooth` e `bluetooth-agent`, apaga `/var/lib/bluetooth/*` e `/storage/roms/backups/bluetooth.tar`, reinicia os dois). `/var/lib/bluetooth` é symlink para `/storage/.cache/bluetooth`. | exit 0 | ASYNC (~2 a 3 s) | `ApiSystem.cpp:727`; `GuiBluetoothDevices.cpp:101` |

Pareamento de áudio (fones): mesmo caminho (`live_devices` + `trust <MAC>`); `module-switch-on-connect` do PipeWire (`PKG/archr/autostart/050-audio`) troca a saída padrão ao conectar.

---

## 3. Volume, saída de áudio, brilho

### 3.1 Volume

| Item | Detalhe | Fonte |
|---|---|---|
| Chave | `audio.volume` (0 a 100, inteiro; seed `60`). | `system.cfg`; `ES/es-core/src/audio/VolumeControl.cpp:270-272` |
| Ler valor atual | O ES conecta ao PulseAudio (PipeWire-pulse) e lê o volume da sink padrão; no start aplica `audio.volume` (padrão 100 se ausente). Equivalente CLI (inferido): `pactl get-sink-volume @DEFAULT_SINK@`. | `VolumeControl.cpp:114-123` |
| Aplicar | `/usr/bin/volume <0-100\|+\|-\|up\|down\|restore>` (`PKG/archr/sources/scripts/volume`): `pactl -- set-sink-volume @DEFAULT_SINK@ N%` e `set_setting audio.volume N`. Passo `+/-` = 5. Limita a 0 a 100. Rápido. O ES grava a chave por si e usa libpulse direto (`setVolume`). | `GuiMenu.cpp:4989-5003` |
| Boot | `050-audio` espera a sink existir e roda `volume <audio.volume>` (padrão 60 ou `DEVICE_VOLUME`); também `amixer sset Master 100%`. | `PKG/archr/autostart/050-audio` |
| Reinício | Não. | |
| Observação | As teclas físicas de volume são tratadas pelo serviço `input.service` (`input_sense`) chamando `volume up/down`, sem o ES. | `PKG/sysutils/system-utils/sources/scripts/input_sense` |

### 3.2 Dispositivo (saída) de áudio

Disponibilidade: executável `set-audio` existe (`ApiSystem.cpp:1975`). Backend PipeWire/WirePlumber via `wpctl` (`PKG/archr/sources/scripts/set-audio`).

| Operação | Comando | Saída | Observações | Fonte |
|---|---|---|---|---|
| Listar | `set-audio list` | Nomes das sinks, um por linha, sem id (ex.: `Built-in Audio Analog Stereo`), extraídos de `wpctl status`. O ES espera `id<TAB>nome`; sem TAB usa a linha toda como valor e rótulo. | `sort -u` (ordem alfabética, nomes repetidos colapsam) | `ApiSystem.cpp:747`; `GuiMenu.cpp:2236-2275` |
| Atual | `set-audio get` | Nome da sink marcada com `*` | | `ApiSystem.cpp:806` |
| Aplicar | `set-audio set '<nome>'` | sem saída; `wpctl set-default <id>` onde id = primeira sink cujo texto contém o nome (`grep -F`, **correspondência por substring**: nomes que contêm outro nome podem escolher a errada). | rápido, mas o ES roda em thread para não travar e toca `checksound.ogg` | `ApiSystem.cpp:810-822` |
| Chave | `audio.device` (o ES grava o nome; `set-audio set` não grava nada). Ninguém reaplica `audio.device` no boot; a persistência real é do WirePlumber (`/storage/.local/state/wireplumber`). | | | `GuiMenu.cpp:2270` |
| Perfis de áudio | `set-audio list-profiles`/`get-profile` retornam vazio; o ES esconde a linha. | | | `set-audio` |
| Reinício | Não. | | | |

### 3.3 Brilho

| Item | Detalhe | Fonte |
|---|---|---|
| Chave | `display.brightness` (percentual 5 a 100; seed `50`); segunda tela `display.brightness2`. | `system.cfg`; `PKG/archr/autostart/006-display` |
| Ler valor atual | `/sys/class/backlight/*/brightness` dividido por `max_brightness` (arredonda). Primeiro `*` em ordem alfabética = tela 1. | `ApiSystem.cpp:1332-1365` |
| Aplicar (modo ES) | Escreve o valor bruto em `/sys/class/backlight/<dev>/brightness` (mín. 1, escala `pct/100*max`), e grava `display.brightness`. Slider do ES: 5 a 100, passo 5. | `ApiSystem.cpp:1367-1387`; `GuiMenu.cpp:1732-1759` |
| Aplicar (modo script, equivalente) | `/usr/bin/brightness set [tela\|all] <0-100>` (força mínimo 5, grava `display.brightness`), `brightness up\|down [tela\|all]`, `brightness get` devolve o valor bruto. | `PKG/archr/sources/scripts/brightness` |
| Boot | `006-display`: `brightness set <display.brightness>`; se vazio ou < 5 usa 10. | |
| Reinício | Não. Rápido. Em RK3326 o root escreve no sysfs, portanto o launcher precisa rodar como root (o ES roda). | |

---

## 4. Idioma, layout de teclado, fuso horário, hostname

| Item | Chave / comando | Valores e leitura | Aplicar e reinício | Fonte ES |
|---|---|---|---|---|
| Idioma | `system.language` (seed `en_US`). | Códigos fixos no código: `ar_YE ca_ES cs_CZ cy_GB de_DE el_GR en_US en_GB es_ES es_MX eu_ES fi_FI fr_FR gl_ES he_IL hu_HU id_ID it_IT ja_JP ko_KR nb_NO nl_NL nn_NO oc_FR pl_PL pt_BR pt_PT ro_RO ru_RU sk_SK sv_SE tr_TR uk_UA vi_VN zh_CN zh_TW`. Vazio = `en_US`. | Só grava a chave. O efeito vem no **próximo start do serviço de UI** (`es_settings` gera o locale com `localedef -i <lang> -f UTF-8 /storage/.config/emulationstation/locale/<lang>.UTF-8` se ainda não existe; exporta `LANG`, `LANGUAGE`, `LOCPATH`). A geração pode levar de dezenas de segundos a minutos no RK3326, **bloqueando o início** do serviço. O ES marca "exitreboot" e sai (ou mostra "REBOOT REQUIRED"). O launcher com i18n próprio só precisa da chave para os scripts/emuladores; se não gerar o locale, `LANG` das ferramentas filhas fica sem o idioma. | `GuiMenu.cpp:1422-1470`; `PKG/ui/emulationstation/sources/es_settings` |
| Layout de teclado | `system.kblayout` (padrão `us`), `system.kbvariant` (padrão `none`). | Listas via `archr-keyboard list-layouts` (linhas `código descrição`, ordenadas pela descrição) e `archr-keyboard list-variants <layout>`. Amostra real (de `IMG/usr/share/X11/xkb/rules/base.lst`): `al Albanian`, `et Amharic`, `ara Arabic`, `eg Arabic (Egypt)`; variantes de `br`: `nativo-epo Esperanto (Brazil, Nativo)`, `dvorak Portuguese (Brazil, Dvorak)`. `archr-keyboard get` imprime `<layout> <variante>`. | `archr-keyboard set <layout> <variante\|none>` só grava as duas chaves (`set_setting`) e tenta `setxkbmap` (**só existe em X11**). **Não há consumidor no Wayland/sway**: `config.desktop` fixa `xkb_layout "us"` e nenhum script lê `system.kblayout`. ES avisa "reboot may be required" mas reboot não muda nada. Ver lacunas. | `GuiMenu.cpp:1480-1560`; `PKG/archr/sources/scripts/archr-keyboard` |
| Fuso horário | `system.timezone` (seed `America/Sao_Paulo`). | Lista: `timeinfo timezones` = `zone1970.tab` coluna 3, ordenada única (real: `Africa/Abidjan,` `Africa/Algiers,` `Africa/Bissau,` `Africa/Cairo,`... um por linha com vírgula no fim; o ES junta tudo via `GetShOutput` e divide por `,`). Atual: chave, senão `timeinfo current_timezone` (= `readlink -f /etc/localtime` sem `/usr/share/zoneinfo/`). `UTC` não está em `zone1970.tab`. | Imediato: `ln -sf /usr/share/zoneinfo/<tz> $(readlink /etc/localtime)` (`/etc/localtime` → `/var/run/localtime`) e gravar a chave. Persistência no boot: `es_settings` (ver 0.5) escreve `/storage/.cache/timezone` e reinicia `tz-data.service`. Alternativa pronta: `archr-config tz <tz>` (escreve `TIMEZONE=<tz>` em `/storage/.cache/timezone` e reinicia `tz-data.service`); **ele não grava `system.timezone`**. Sem reboot. | `GuiMenu.cpp:1566-1585`; `PKG/archr/sources/scripts/timeinfo`, `archr-config:282` |
| Hostname | `system.hostname` (seed `archr`; default interno do ES se ausente: `BATOCERA`, resíduo do upstream). | Texto livre (ES usa campo de texto). Sem validação. | Só grava a chave. **Aplicado no boot** por `network-base-setup` (`/proc/sys/kernel/hostname` + `avahi-set-host-name`), que **acrescenta `-<4 primeiros hex do machine-id>`** (o nome efetivo é `archr-ab12`). Aplicação ao vivo: replicar `echo <nome>-<sufixo> > /proc/sys/kernel/hostname` e `avahi-set-host-name`. Sem validação de caracteres (RFC 1123: letras, dígitos, `-`; até 63). | `GuiMenu.cpp:5221`; `PKG/sysutils/systemd/scripts/network-base-setup` |

---

## 5. Mapeamento de controle e vibração

### 5.1 Mapeamento (`es_input.cfg`)

| Aspecto | Detalhe | Fonte |
|---|---|---|
| Arquivo do usuário | `/storage/.config/emulationstation/es_input.cfg`. A cópia semente (118 KB, centenas de controles pré-mapeados) vive em `/usr/config/emulationstation/es_input.cfg` e é copiada para o diretório do usuário no primeiro boot (`rsync --ignore-existing /usr/config/* /storage/.config/`, `PKG/sysutils/systemd/scripts/userconfig-setup:13`); o "caminho compartilhado" do ES (`<exe>/es_input.cfg`, com `<exe>` = `/usr/bin`) não existe, então na prática só vale o arquivo do usuário. Busca: usuário exato, compartilhado exato, usuário aproximado, compartilhado aproximado. Como a semente não é re-sincronizada em updates, controles novos só entram se o usuário mapear. | `ES/es-core/src/InputManager.cpp:889-920, 1062` |
| Entrada de mapeamento | `GuiControllersSettings` → "CONTROLLER MAPPING" → `GuiDetectDevice` → `GuiInputConfig`. Por controle: 25 entradas na ordem Up, Down, Left, Right, Start, Select, A(east), B(south), X(north), Y(west), LeftShoulder, RightShoulder, LeftTrigger, RightTrigger, LeftThumb, RightThumb, LeftAnalog Up/Down/Left/Right, HotKeyEnable, RightAnalog Up/Down/Left/Right. Botões "skippable" podem ser ignorados segurando um botão. | `GuiControllersSettings.cpp:75-90`; `ES/es-core/src/guis/GuiInputConfig.cpp:50-80` |
| Como grava | `InputManager::writeDeviceConfig` carrega o XML, **remove a entrada com o mesmo `deviceGUID` e `deviceName`**, anexa a nova e salva com `pugixml`. Cria também `/storage/.config/emulationstation/es_last_input.cfg` (só a última), dispara `config-changed` e `controls-changed`, roda comandos `onfinish`. | `InputManager.cpp:955-1000` |
| Formato (real, de `IMG/usr/config/emulationstation/es_input.cfg`, `r36s_Gamepad`) | `<inputConfig type="joystick" deviceName="r36s_Gamepad" deviceGUID="19003982010000008811000088010000">` com `<input name="a" type="button" id="1" value="1" />`, `<input name="leftanalogup" type="axis" id="1" value="-1" />`, `name` ∈ `a b x y up down left right start select leftshoulder rightshoulder lefttrigger righttrigger leftthumb rightthumb leftanalog{up,down,left,right} rightanalog{up,down,left,right} hotkeyenable`; `type` ∈ `button axis hat key`. | `es_input.cfg` linha 1735 |
| Seleção de jogador | Chaves do **es_settings.cfg** (não do system.cfg): `INPUT P<n>NAME`, `INPUT P<n>GUID`, `INPUT P<n>PATH` (n = 1..MAX_PLAYERS), "DEFAULT" para padrão. Não afeta `system.cfg`. | `GuiControllersSettings.cpp` (loop de players) |
| Toggle global | `system.autohotkeys` (`1`/`0`, seed `1`): "AUTOCONFIGURE RETROARCH HOTKEYS". | `GuiControllersSettings.cpp:69` |

**O que é regenerado a partir do `es_input.cfg` (consumidores):**

| Consumidor | Quando roda | O que faz |
|---|---|---|
| `mkcontroller` (`PKG/virtual/emulators/scripts/mkcontroller`) | Boot (`autostart/001-controller`) e na partida do `input.service` (`input_sense` linha ~302) | Lê o primeiro controle de `control-gen` (`DEVICE`) em `es_input.cfg` via `xmlstarlet` e escreve `/storage/.config/profile.d/098-controller` (`DEVICE_BTN_SOUTH/EAST/.../DPAD/AL/AR/MODE`). Esse arquivo alimenta hotkeys (`input_sense`), scripts de nuvem e emuladores. |
| `controller-layout` / `create_controller_db` (`PKG/archr/profile.d/100-gamecontroller-functions`) | A cada lançamento de jogo (via `runemu.sh`/`setsettings.sh`, layout vindo de `controller_layout` do cfg: `xbox`, `xbox_swap_shoulders_triggers` ou Nintendo) | Filtra `es_input.cfg` para os controles conectados (`/tmp/es_input.cfg`) e gera `gamecontrollerdb.txt` para SDL/standalone. |
| `control-gen` | boot (`gptokeyb`) | Gera `control.ini`. |
| Backup | `backuptool` | **Não inclui `es_input.cfg`**: usa `/storage/.emulationstation/es_*.cfg` (caminho morto); ver lacunas. |

**Efeito imediato no pós-mapeamento:** nada regenera sozinho. `098-controller` só atualiza no próximo boot (ou ao reiniciar `input.service`, que reexecuta `mkcontroller`). O ES não chama `mkcontroller`. O launcher, após gravar o mapeamento, deve executar `mkcontroller` e `systemctl restart input.service` (inferido; testar) para os hotkeys usarem os novos botões. Jogos pegam o novo mapeamento no próximo lançamento.

O launcher precisa de leitura **e** escrita de `es_input.cfg` no formato acima (ele também é lido por scripts em shell com `xmlstarlet`; manter exatamente atributos `name/type/id/value`).

### 5.2 Vibração (rumble)

| Aspecto | Detalhe | Fonte |
|---|---|---|
| Chave | `rumble.enabled` (`1`/`0`). Boot: `PKG/hardware/quirks/autostart/020-rumble` procura `rumble_enable` em `/sys/devices/platform` e, se existir, grava a chave no arquivo (ou semeia `1`). | |
| Detecção no ES | `getRumblePath()` procura em `/sys/devices/platform` um diretório com `retroid-pocket-gamepad` ou `archr` no nome contendo `rumble_enable`. Sem ele, **a linha "ENABLE RUMBLE" não aparece**. | `ApiSystem.cpp:685-709` |
| Aplicar | `echo 1\|0 > <rumblePath>` e gravar a chave. | `GuiControllersSettings.cpp:92-105` |
| **Estado no RK3326** | O driver do joypad (`odroidgo3-joypad`, `devices/RK3326/patches/linux/004-input-drivers.patch`) expõe só `rumble_period`, `rumble_boost_strong`, `rumble_boost_weak`. **Não existe `rumble_enable`** em nenhum patch ou DTS. No R36S o DTS ainda força o pino do motor em nível baixo (`rumble-en-hold-low`, `rk3326-gameconsole-r36s.dts`), de modo que a vibração é desativada no hardware. Resultado: **o item P0 "vibração" não tem backend em RK3326** (ES não mostra, o quirk não faz nada). | |

---

## 6. Emulador e núcleo por sistema e por jogo; opções do `es_features.cfg`

### 6.1 Emulador/núcleo

| Aspecto | Detalhe | Fonte |
|---|---|---|
| Origem da lista | `es_systems.cfg`: `/storage/.config/emulationstation/es_systems.cfg` → symlink para `/usr/config/emulationstation/es_systems.cfg` (gerado por `mk_es_systems` em `PKG/virtual/emulators/package.mk:1443`; reaponta a cada update em `archr/sources/post-update:30-35`). 125 sistemas na imagem. Estrutura: `<system><name>…</name><emulators><emulator name="retroarch"><cores><core default="true">opera</core>…`. `default="true"` marca o padrão. O `<command>` é `/usr/bin/runemu.sh %ROM% -P%SYSTEM% --core=%CORE% --emulator=%EMULATOR% --controllers="%CONTROLLERSCONFIG%"`. | `IMG/usr/config/emulationstation/es_systems.cfg` |
| Chaves por sistema | `<sistema>.emulator` e `<sistema>.core`. Vazio/`auto` = padrão. | `GuiMenu.cpp:4053-4060` (patch 0004 grava no `SystemConf`) |
| Chaves por jogo | `<sistema>["<arquivo>"].emulator` e `.core` (mesma regra de nome da seção 0.1). | `FileData.cpp:1499-1515` |
| Leitura | `getEmulator()/getCore()` leem a chave; se o emulador/núcleo gravado não existir mais em `es_systems.cfg`, caem no padrão (`FileData.cpp:1420-1497`). | |
| Seletor na UI | `popSpecificConfigurationGui` (`GuiMenu.cpp:5935`): lista `AUTO` + todo par `emulador/núcleo`; o valor do item é `"<emulador>/<núcleo>"` (rótulo `emulador: núcleo`, ou só `emulador` quando os nomes coincidem; emulador sem núcleo vira só `emulador`). Ao escolher, grava as **duas chaves** (`.emulator`, `.core`) e **reconstrói a tela** (as opções dependem do par). Tela alternativa mais simples: `openSystemEmulatorSettings` (`GuiMenu.cpp:3980`) com dois seletores. O sistema só aparece em "PER SYSTEM ADVANCED CONFIGURATION" se `hasFeatures() || hasEmulatorSelection()` (`GuiMenu.cpp:3927`). | |
| Passagem ao script | `%SYSTEM%`, `%CORE%`, `%EMULATOR%` substituídos no `<command>`; quando o par é forçado e o comando não tem os marcadores, anexa `-emulator X -core Y` (`FileData.cpp:550-568`). `runemu.sh` extrai por `-P`, `--core=`, `--emulator=`. Núcleo/emulador resolvidos são os do padrão se a chave é vazia. | `FileData.cpp:515-590` |
| Reinício | Não. Efeito no próximo lançamento. | |

### 6.2 `es_features.cfg`: como o ES lê

| Aspecto | Detalhe | Fonte |
|---|---|---|
| Arquivo | `/storage/.config/emulationstation/es_features.cfg` (symlink para `/usr/config/emulationstation/es_features.cfg`, gerenciado por update); fallback `<exe>/es_features.cfg` (`/usr/bin`, inexistente na prática). Adicionais opcionais `es_features_*.cfg` no mesmo diretório são mesclados. Fonte no repositório: `PKG/ui/emulationstation/config/common/es_features.cfg`; no build, `package.mk` remove todas as linhas com `vulkan` se não há Vulkan (`sed -i '/vulkan/d'`, vide diferença entre repo e `IMG`). | `ES/es-app/src/CustomFeatures.cpp:155-200` |
| Estrutura | `<features><emulator name="…" features="…"><features>…<feature>…</features><cores><core name="…" features="…"><features>…</features><systems><system name="…">…`. Um `name` pode listar vários separados por vírgula. Hoje: 23 emuladores, 163 núcleos, 260 `<feature>`, 789 `<choice>`; **nenhum** `<sharedFeatures>`/`<globalFeatures>` no arquivo (portanto não há opções globais definidas por arquivo). | `CustomFeatures.cpp:200-335` |
| `<feature>` | Atributos: `name` (rótulo, obrigatório), `value` (sufixo da chave; se ausente = `name` com espaços trocados por `_`), `description`, `group` (cabeçalho), `submenu` (agrupa em subtela), `preset`, `preset-parameters`, `order`. Filhos `<choice name="rótulo" value="valor">` (valor ausente = nome com `_`). Só entra se tiver `choice` **ou** `preset`. | `CustomFeatures.cpp:406-470` |
| Chave gravada | `<configName>.<value>` (`configName` = nome do sistema, ou `sistema["jogo"]`, ou `global`). O **núcleo/emulador não entra na chave**: duas características de núcleos diferentes com o mesmo `value` compartilham a chave. | `GuiMenu.cpp:3059-3063` |
| Lista de valores no widget | Item de lista: primeira opção `AUTO` (valor `""`, que remove a chave), depois cada `choice`. Selecionado = `choice.value == valor da chave`. Se houver valor herdado (global ou sistema), a descrição mostra "Current setting : …". | `GuiMenu.cpp:3383` |
| Presets | `switch`/`switch_default_off` (`1` ou removido), `switchon`/`switch_default_on` (removido ou `0`), `switchoff`/`switch_default_off_reverse_value` (removido ou `1`), `switchauto` (removido/`1`/`0`), `slider`/`sliderauto` (`preset-parameters` = `min max passo [default] [sufixo]`; **grava `std::to_string(float)`, ex. `50.000000`**), `input`, `password`, `image`, `video`, `folder`, `document`, `files`, `shaders`/`shaderset` (lista de `getshaders`), `videofilters` (lista de `getfilters`), `decorations`/`bezel` (diretórios em `/storage/roms/bezels` e `/tmp/overlays/bezels`), `videomode(s)` (`archr-resolution listModes`), `runners`, `hidden`. O arquivo atual **não usa `preset`**: todas as opções são listas de `choice`. | `GuiMenu.cpp:3056-3300` |
| Flags do atributo `features=` | `ratio rewind smooth shaders videofilters pixel_perfect decoration latency_reduction game_translation fullboot emulated_wiimotes screen_layout internal_resolution videomode colorization autocontrollers autosave netplay cheevos padtokeyboard`. Cada flag liga uma linha **fixa no código** (não vem do arquivo), listada em 6.3. Hoje só aparecem as de `retroarch` (`decoration, ratio, smooth, shaders, pixel_perfect, latency_reduction, game_translation`) e as por núcleo (`netplay, rewind, autosave, cheevos, decoration, colorization`). | `CustomFeatures.cpp:30-60` |
| Montagem da tela do sistema/jogo | `systemData->getCustomFeatures(emulador, núcleo)` agrega emulador + núcleo + sistema; `addFeatures(...)` ordena por `group` e `submenu`. Apêndice A traz o dump completo. | `GuiMenu.cpp:5935-6460`, `3468-3556` |

### 6.3 Opções fixas do código por flag (todas gravam `<configName>.<chave>`)

| Flag | Rótulo na UI | Chave (`<cfg>` = sistema, jogo ou `global`) | Valores (valor gravado) | Fonte |
|---|---|---|---|---|
| `ratio` | GAME ASPECT RATIO | `<cfg>.ratio` | `auto` (remove) ou valores de `LibretroRatio` (ex. `4/3`, `core`, `16/9`...; ver tabela do ES em `ES/es-app/src/LibretroRatio.cpp`) | `GuiMenu.cpp:6010` (sistema/jogo), `3640` (global) |
| `smooth` | SMOOTH GAMES | `<cfg>.smooth` | `auto`, `1`, `0` | `GuiMenu.cpp:6026` |
| `rewind` | REWIND | `<cfg>.rewind` | `auto`, `1`, `0` | `GuiMenu.cpp:6035` |
| `autosave` (e sem feature própria de mesmo nome) | AUTO SAVE/LOAD ON GAME LAUNCH | `<cfg>.autosave` | `auto`, `1`, `0` | `GuiMenu.cpp:6044` |
| `shaders` | SHADER SET | `<cfg>.shaderset` | `auto`, `none`, ou nome da saída de `/usr/bin/getshaders` (lista separada por vírgula) | |
| `shaders` (no ARCHR a tela de filtro de vídeo reusa a flag) | VIDEO FILTER | `<cfg>.videofilter` | `auto`, `none`, ou saída de `/usr/bin/getfilters` | |
| `pixel_perfect` | INTEGER SCALING | `<cfg>.integerscale` | `auto`, `1`, `0` | `GuiMenu.cpp:6125` |
| `decoration` | bezel set | `<cfg>.bezel` | `auto`, `none`, nome de diretório | |
| `latency_reduction` | LATENCY REDUCTION (subtela) | `<cfg>.runahead` (`""`=auto, `0`,`1`..`6`), `<cfg>.secondinstance` (`""`,`1`,`0`), `<cfg>.video_frame_delay_auto` (`""`,`1`,`0`) | | `GuiMenu.cpp:2811-2832` |
| `colorization` | COLORIZATION (gb/gbc) | `<cfg>-renderer.colorization` (**note o `-renderer`**) | `auto`, `none`, e 126 nomes de paleta (ex. `GB - DMG`, `GBC - Blue`, `SGB - 1A`, `TWB01 - 756 Production`) | `GuiMenu.cpp:6208` |
| `fullboot` | FULL BOOT (ps2) | `<cfg>.fullboot` | `auto`, `1`, `0` | `GuiMenu.cpp:6359` |
| `emulated_wiimotes` | EMULATED WIIMOTES | `<cfg>.emulatedwiimotes` | `auto`, `1`, `0` | |
| `screen_layout` | CHANGE SCREEN LAYOUT (3ds) | `<cfg>.layout_option` | `auto`, `2` (large), `3` (side by side) | `GuiMenu.cpp:6383` |
| `internal_resolution` | INTERNAL RESOLUTION | `<cfg>.internalresolution` | `auto`, `0` (1:1), `1`,`2`,`3`,`4`,`5`,`8`,`10` | `GuiMenu.cpp:6395` |
| `autocontrollers` | AUTOCONFIGURE CONTROLLERS | `<cfg>.disableautocontrollers` | `""` (auto), `0` (ligado), `1` (desligado) | `GuiMenu.cpp:6422` |
| `videomode` | VIDEO MODE | `<cfg>.videomode` | `auto` ou id de `archr-resolution listModes` | |
| (sempre, "SYSTEM OPTIONS") | CPU SCALING GOVERNOR | `<cfg>.cpugovernor` | `default` ou governadores | `GuiMenu.cpp:2910-2935` |
| (sempre) | GPU SCALING GOVERNOR | `<cfg>.gpuperf` | `default` ou governadores | `GuiMenu.cpp:2937-2965` |
| (sempre, se mangohud) | MANGOHUD | `<cfg>.archr.mangohud.enabled` | `""`, `1`, `0` | `GuiMenu.cpp:2968` |
| (sempre, se wlr-randr) | DISPLAY MODE | `<cfg>.display_mode` | `default` ou modo | |
| só jogo | LAUNCH THIS GAME AT STARTUP | `global.bootgame.path`, `global.bootgame.cmd` | caminho / comando | `GuiMenu.cpp:6444` |

Observação sobre `description`/`group` do arquivo: o ES traduz com `pgettext("game_options", …)`; o launcher usará o `name` cru ou seu catálogo.

Consumo: `setsettings.sh` (`PKG/archr/sources/scripts/setsettings.sh`) lê cada opção com `game_setting <opção>` = `get_setting <opção> <sistema> <rom>` e gera o `retroarch.cfg` temporário/arquivos dos standalone antes do jogo. Portanto qualquer chave nova do `es_features.cfg` só funciona se existir tratamento correspondente nos scripts de cada emulador (`PKG/emulators/**/scripts/start_*.sh`). O launcher só grava a chave.

---

## 7. Autosave e savestates

| Opção | Chave | Valores | Escopo | Consumidor | Fonte ES |
|---|---|---|---|---|---|
| Auto salvar/carregar (global) | `global.autosave` | `1` ou removido (o ES grava `1` ou `""`); seed `1` | global | `setsettings.sh:790-825` (`set_autosave`): argumento `-autosave 0\|1` do `runemu.sh` vence; senão `game_setting autosave` com valores `1`, `2` ou `3` = liga | `GuiMenu.cpp:3611-3615` |
| Auto salvar/carregar (sistema/jogo, quando o núcleo tem a flag `autosave`) | `<sistema>.autosave`, `<sistema>["jogo"].autosave` | `auto` (remove), `1`, `0` | sistema/jogo | idem | `GuiMenu.cpp:6044-6052` |
| Savestates incrementais | `global.incrementalsavestates` | `""` (um novo arquivo a cada salvar), `0` (incrementa o slot em jogo novo; seed `0`), `2` (usa o slot atual). Atenção: `1` não é usado ("1 é SIM, auto também"). | global | `setsettings.sh` ~ linha 775 (`savestate_auto_index`, `savestate_max_keep`) | `GuiMenu.cpp:3617-3624`; `SystemConf.cpp` `getIncrementalSaveStates` |
| Gerenciador de savestates antes do jogo | `global.savestates` | `auto` (não), `1` (sempre), `2` (se não vazio); seed `1` | global | **só o ES** (tela `GuiSaveState` antes de lançar). Ao escolher um slot, o ES acrescenta à linha de comando `-state_slot N`, `-autosave 1\|0` e `-state_file "<arquivo>"` (`SaveState.cpp:91-138`). O launcher que quiser o mesmo recurso terá de montar esses argumentos para `runemu.sh`. | `GuiMenu.cpp:3628-3633` |
| Diretório | `/storage/roms/savestates` (`Paths.cpp`, `mSaveStatesPath`); RetroArch: `savestate_directory = <SNAPSHOTS>/<sistema>` | | | | |
| Reinício | Não. | | | | |

---

## 8. GPU, suspensão, troca de dispositivo, governador, GPU perf, turbo, resfriamento

| Item | Chave / comando | Valores e leitura | Aplicar e reinício | Dispositivos | Fonte ES |
|---|---|---|---|---|---|
| Driver de GPU | Script `/usr/bin/gpudriver` (`PKG/graphics/gpudriver/sources/bin/gpudriver`); chave `gpu.driver`. | Opções: `gpudriver --options` → `panfrost libmali` (uma linha, separadas por espaço). Atual: `gpudriver` sem argumento → imprime o driver (**efeito colateral: se a chave está vazia, grava `libmali`**). | `gpudriver <panfrost\|libmali>`: `set_setting gpu.driver`, `sync` (e placeholders `@DTB_OVERLAY_*@`, vazios no RK3326). **Reinício obrigatório.** ES mostra "GPU driver will be switched on next reboot" com "Reboot now". No boot `gpudriver --start` carrega o driver, com sentinela `/storage/.config/gpudriver.attempting` e fallback automático para o outro driver se não houver `bind`; log em `/storage/.config/gpudriver.log`. Rápido. | A linha só aparece se `/usr/bin/gpudriver` existe | `GuiMenu.cpp:2081-2100` |
| Modo de suspensão | Chave `system.suspendmode`; script `/usr/bin/suspendmode <modo>` | Lista: `default` + tokens de `/sys/power/state` menos `disk` (tipicamente `freeze mem`, talvez `standby`); **`off` não aparece na lista** embora seja o valor que o quirk do RK3326 grava (`030-suspend_mode`: se a chave está vazia, roda `suspendmode off` no boot). O ES inclui o valor atual na lista se não estiver nela (`GuiMenu.cpp:2122-2123`). Atual: chave; vazio = `default`. | O script **compara com o valor gravado no arquivo** e só age se for diferente; por isso o launcher **não deve gravar a chave antes** de chamar `suspendmode`. Ele escreve `/storage/.config/sleep.conf.d/sleep.conf` (`AllowSuspend=no` para `off`; `SuspendState=...` para os demais), grava a chave e faz `systemctl restart systemd-logind`. `default`: apaga a chave e roda o quirk (no RK3326 resulta em `off` + chave removida). Sem reboot, efeito imediato. | Bloco `#if RK3326` (compilado) | `GuiMenu.cpp:2107-2130`; `PKG/archr/sources/scripts/suspendmode` |
| Troca de dispositivo R33S/R36S | `/usr/bin/device-switch` | `device-switch` imprime o atual (lê `/sys/firmware/devicetree/base/archr,device_switch/this`, ex. `r36s`); `device-switch --options` imprime os alternativos separados por espaço (do grupo r36 no DTS: `r33s r36s rgb10x`, o atual vem de `this`; o `ls` exclui `name` e `this`). | `device-switch <alvo>`: resolve o nome do DTB, remonta `/flash` rw, edita `extlinux.conf` (`FDT`), grava `/flash/device.name`, troca o conjunto de `/storage/remappings`, `sync`, remonta `ro`. **Reinício obrigatório.** Exit 2 (nome não resolve) / 3 (dtb ausente). **O ES não detecta a falha** (ver 0.3). | `DEVICE_DTB_SWITCH=true` | `GuiMenu.cpp:1866-1895` |
| Governador de CPU | Chave `system.cpugovernor` (seed `ondemand`). | Lista: `default` + `scaling_available_governors` da `policy0` (RK3326: tipicamente `conservative ondemand userspace powersave performance schedutil`). Atual: chave (`default` se vazia). | `sh -lc ". /etc/profile.d/099-freqfunctions; <governador>"`. Só existem funções para **`performance`, `ondemand`, `schedutil`, `powersave`**: `conservative`, `userspace` e `default` dão "command not found". O ES roda sem checar nada. O `008-perfmode` reaplica no boot (`ondemand` se vazio) e o `runemu.sh` restaura ao sair do jogo. Sem reboot. | todos | `GuiMenu.cpp:1972-1996`; `ApiSystem.cpp:763` |
| GPU perf | Chave `system.gpuperf` | Lista: `default` + `$GPU_FREQ/available_governors` (`get_available_gpu_governors`). Atual: chave. | `sh -lc ". /etc/profile.d/099-freqfunctions; gpu_performance_level <v>"`: `auto`/`default` → ondemand; `performance` → governador performance + `min_freq` no máximo; outro → `set_gpu_gov <v>`. `008-perfmode` reaplica no boot; `powerstate.sh` reaplica ao trocar bateria/carregador. Sem reboot. | todos | `GuiMenu.cpp:1998-2030` |
| Turbo (CPU overclock) | Chave `enable.turbo-mode` (`1`/`0`, seed `0`); `/usr/bin/turbomode enable\|disable` | Atual: chave. | `turbomode` já grava a chave (`set_setting`), escreve `/sys/devices/system/cpu/cpufreq/boost` e `set_cpu_max_freq highest`. Boot: `095-turbo-mode`. Sem reboot. O ES chama o script e depois grava a chave também (redundante). | `DEVICE_TURBO_MODE=true` (RK3326) | `GuiMenu.cpp:2026-2043` |
| Resfriamento | Chave `cooling.profile` (`auto`,`quiet`,`moderate`,`aggressive`,`custom`; seed `quiet`); por jogo `<cfg>.cooling.profile`. | | `systemctl restart fancontrol` | **`DEVICE_HAS_FAN` não existe no RK3326: o item não aparece e não há backend.** O ES grava a chave em memória e reinicia o serviço antes do flush (leitura velha). | `DEVICE_HAS_FAN=true` | `GuiMenu.cpp:1950-1970` |
| GPU overclock | `enable.gpu-overclock` | | `.../bin/gpu_overclock` | Nenhum quirk do RK3326 define `DEVICE_GPU_OVERCLOCK` | `GuiMenu.cpp:2045-2080` |

---

## 9. Atualização do sistema (`archr-update`)

Backend: `/usr/bin/archr-update` (sh; `PKG/archr/sources/scripts/archr-update`), baseado em `pacman` contra `https://github.com/archr-linux/archr-repo/releases/download/repo-<canal>`. Disponibilidade na UI: executável `archr-update` existe (`ApiSystem.cpp:1987`).

| Operação | Comando | Saída e retorno | Bloqueia? | Fonte ES |
|---|---|---|---|---|
| Verificar | `archr-update check` | Sem rede: `Network connection unavailable.`, exit 1 (testa `ping -c 1 -W 3 8.8.8.8`). Reescreve `/etc/pacman.d/mirrorlist`, semeia o banco local na 1ª vez (`curl` de `archr-localdb.tar.gz`), `pacman -Sy`. Sem atualização: `No updates available.` **e exit 1**. Com atualização: `Updates available: N package(s).` + saída de `pacman -Qu` (`pacote versão -> versão`), exit 0. Falha de rede/sync: `Failed to sync repo metadata. Check network.`, exit 1. **Exit 1 mistura "nada novo" com "falhou".** O ES só define a "versão" (`mUpdateVersion`) quando a saída tem **exatamente 1 linha** (`GuiUpdate.cpp` `threadPing`) e só abre o diálogo de confirmação se ela não estiver vazia (`GuiUpdate.cpp:167`). Com atualização disponível a saída tem no mínimo 2 linhas (cabeçalho + `pacman -Qu`), portanto **o fluxo de atualização do ES hoje não mostra confirmação e nunca inicia** (a tela fica inerte); com `updates.force=1` e "No updates available." ele pergunta "atualizar para a versão No updates available.". | **ASYNC** (dezenas de s; ES faz em thread com spinner) | `ApiSystem.cpp:346-371`; `GuiUpdate.cpp:96-110` |
| Atualizar | `archr-update` (= `update`): `pacman -Syu --noconfirm` | Saída crua do pacman linha a linha; exit do pacman. **Sem marcador de progresso**: o ES procura `>>> NN%` nas linhas para a barra (`GuiUpdate.cpp:44-55`) e `archr-update` nunca imprime isso, então a barra fica indeterminada. | **ASYNC longo (minutos)**, streaming; ES grava `system-upgrade.log` em `/var/log` | `ApiSystem.cpp:209-240` |
| Info | `archr-update info` | `OS:`, `Build:`, `Channel:`, `Mirror:`, `Pacman:` | rápido | |
| Canal | Chave `updates.branch`. **Valores aceitos pelo script: `stable`, `next`, `dev`**; qualquer outro (inclusive `auto`, valor semente) vira `stable`. | | | |
| Ligar/forçar | `updates.enabled` (seed `1`; só decoração no ES), `updates.force` (`1` faz `canUpdate` devolver "sim" mesmo com exit 1). | | | `GuiMenu.cpp:1321-1323` |
| Depois de atualizar | ES muda o botão para "APPLY UPDATE" que faz `quitES(RESTART)` (**reinicia só o ES, `touch /tmp/restart.please`; não reinicia o sistema**), embora o aviso diga "REBOOT TO APPLY". Pacotes de kernel/DTB/`/flash` exigem reboot de verdade. O launcher deve oferecer reboot. | | | `GuiMenu.cpp:1338-1343`; `GuiUpdate.cpp:61-66` |

Defeito: a lista de canais da UI do ES é `auto`→"auto", `stable`→"release" e `nightly`→"nightly" (`GuiMenu.cpp:1325-1329`; no código o rótulo `release` guarda o valor `stable`). `nightly` não existe no script (cai em `stable`), `next` e `dev` não aparecem na UI. O launcher deve oferecer `stable`, `next`, `dev`.

---

## 10. Menu de energia, suspender, aviso de bateria baixa

| Ação | Comando / mecanismo | Observações | Fonte |
|---|---|---|---|
| Desligar | ES: `quitES(SHUTDOWN)`: dispara scripts `quit shutdown`/`shutdown`, encerra o laço, grava metadados (gamelists) e roda `shutdown -P -h now`; cria `/tmp/shutdown.please`. "FAST SHUTDOWN" pula o salvamento de metadados. | Para o launcher: `systemctl poweroff` (ou `shutdown -P now`) depois de fechar o próprio estado. | `ES/es-core/src/utils/Platform.cpp:196-203, 216-278` |
| Reiniciar | `shutdown -r now` (`/tmp/reboot.please`). | idem com `systemctl reboot`. | idem |
| Reiniciar a UI | `quitES(QUIT)`: sai do processo; o serviço `Restart=always` o relança. | | `GuiMenu.cpp:5819-5826` |
| Suspender | O item "SUSPEND SYSTEM" **só aparece se existirem `/usr/sbin/pm-suspend` e `pm-is-supported`** (`ApiSystem.cpp:1990`), que **não estão na imagem**; portanto não aparece hoje. Caminho real do aparelho: botão de energia → `input.service` chama `archr-fake-suspend power` (`input_sense:399`); tampa → `archr-fake-suspend lid close\|open`. | `archr-fake-suspend <power\|lid> [open\|close]` (sh/bash): **só age quando a suspensão por hardware está desligada** (`system.suspendmode == off`, o padrão do RK3326), senão sai 0 e deixa o logind suspender. Faz: cria flag, liga `display_off` (DPMS via `swaymsg output * power off` se `system.suspend.dpms=1`; sempre `bl_power=4` no backlight), muta áudio (`pactl set-sink-mute`), governadores em powersave (guarda em `sleep.cpugovernor`/`sleep.gpugovernor`), estaciona cores se `system.suspend.park_cores=1`, bloqueia entrada (`evtest --grab` nos dispositivos fora da lista de exceção), desliga LEDs; espera `system.shutdown_delay` s (jogo rodando: `system.shutdown_delay_running_game`; seed 900 = 15 min) e então desliga, **a menos que HDMI conectado ou carregando**. Para retomar: nova chamada `archr-fake-suspend power` (alterna) ou `lid open`. **Bloqueia pelo tempo de espera: rodar em segundo plano (`&`), como o `input_sense` faz.** Suspender a partir do launcher = `archr-fake-suspend power &` (se o modo é `off`) ou `systemctl suspend` (se há suspensão real). | `archr-fake-suspend`; `input_sense:395-410` |
| Atrasos de desligamento | Chaves `system.shutdown_delay`, `system.shutdown_delay_running_game` (segundos; ES mostra 0 a 30 min, passo 1 min, só no ramo não-RK3326 que **não é compilado aqui**), `system.suspend.park_cores`, `system.suspend.dpms` (`1`/`0`). No RK3326 o ES não expõe esses controles. | Os scripts leem as chaves a cada evento, sem reinício. | `GuiMenu.cpp:2132-2190` (ramo `#else`, inativo) |
| Aviso de bateria baixa | Chaves `system.battery.warning` (`1`/`0`, seed `0`: aviso falado) e `system.battery.warning_threshold` (0 a 50 %, seed `25`). | Quem consome: `powerstate.sh` (serviço independente do ES, lê as chaves em cada volta do laço, sem reinício). Quando a bateria ≤ limiar e descarregando, a cada ~40 s: se `DEVICE_LED_CONTROL=true` e `DEVICE_BATTERY_LED_STATUS` falso, pisca o LED (`led_flash`); senão, se `system.battery.warning=1`, fala `say "BATTERY AT N%"`. Nenhuma notificação visual vem do backend: o ES só desenha o ícone de bateria. O launcher deve desenhar seu próprio aviso lendo `/sys/class/power_supply/*/capacity` e `status`. | `PKG/sysutils/powerstate/sources/powerstate.sh`; `GuiMenu.cpp:5083-5100` |

---

## 11. Armazenamento: automount, armazenamento unificado, ejetar microSD

Bloco inteiro oculto em aparelhos com `DEVICE_MMC_EJECT=false` (seção 0.4). Serviço: `archr-automount.service` (`Type=oneshot`, `RemainAfterExit=yes`, `ExecStart=/usr/bin/automount`).

| Item | Chave / comando | Valores | Aplicar | Bloqueia? | Fonte ES |
|---|---|---|---|---|---|
| Autodetectar cartão de jogos | `system.automount` (`1` seed / `0`) | `0` → `automount` só usa o interno (`start_ms internal`) | **Gravar a chave antes**, depois `systemctl restart archr-automount`. O ES chama o restart com a chave só em memória (`GuiMenu.cpp:1904-1909`): o script lê o valor antigo. | **Pode demorar** (varre `blkid`, monta, às vezes `fsck`): ASYNC. Primeiro desmonta `/storage/games-external` e `/storage/roms`, portanto o launcher precisa soltar qualquer arquivo aberto em `/storage/roms`. | `GuiMenu.cpp:1901-1910` |
| Armazenamento unificado (merged) | `system.merged.storage` (`1`/`0`, seed `0`) | Só mostrado quando existe `/storage/.ms_supported` (cartão externo ext4/btrfs) e `system.automount=1` | Idem (gravar e reiniciar o serviço). `1` monta `overlay` (lower/upper conforme `system.merged.device`) em `/storage/roms`; `0` ou cartão FAT/exFAT/NTFS (`/storage/.ms_unsupported`) faz `mount --bind /storage/games-external/roms /storage/roms`. | ASYNC | `GuiMenu.cpp:1911-1920` |
| Cartão primário do merged | `system.merged.device` (`default`/`external`/`internal`) | | O ES executa `systemctl restart archr-automount <valor>`: o `<valor>` vira um 2º **nome de unidade** e é ignorado (a unidade inexistente só gera erro); o script lê a chave do arquivo, que ainda não foi gravada. Fazer: gravar e reiniciar sem argumento. ES exige confirmação ("pode perder acesso a jogos"). | ASYNC | `GuiMenu.cpp:1921-1940` |
| Ejetar microSD | `umount -f /storage/roms; umount -f /storage/games-external` (ou só `/storage/roms` se não há `/storage/.ms_supported`) | Depois o ES mostra "You may now remove the card." Não desliga o automount: um novo `restart archr-automount` ou reboot remonta. Falha silenciosa se algo estiver aberto (`-f`). | Rápido, mas pode falhar com "busy". | ASYNC curto | `GuiMenu.cpp:1940-1950` |
| Estado atual | `grep` em `/proc/mounts` (`/storage/roms`, `/storage/games-external`), existência de `/storage/.ms_supported` / `/storage/.ms_unsupported` | | | | `automount` |

---

## 12. Senha de root, backup, restauração, restauração de fábrica

Todas as telas ficam em "SYSTEM SETTINGS" (`GuiMenu.cpp:2753-2790`) e só em modo UI completo.

| Item | Chave / comando | Detalhe | Bloqueia? Reinício? | Fonte ES |
|---|---|---|---|---|
| Senha de root | Chave `root.password` (texto puro no cfg; seed `archr`; o ES mostra `linux` se vazia) e `setrootpass <senha>` | `setrootpass`: se a senha já bate com o hash, sai sem fazer nada; senão gera hash sha512 (`cryptpw`), edita `/storage/.cache/shadow`, `smbpasswd -sa root`, `syncthing generate --gui-user root --gui-password`, grava `root.password`, reinicia syncthing/simple-http-server se ativos. Também roda no boot (`autostart/007-rootpw`). **O ES monta `"setrootpass " + senha` num shell**: senha com espaço, `;`, `$` ou aspas quebra ou injeta. O launcher deve passar a senha como **argumento único via argv**, sem shell. | ~6 s (hash + samba + syncthing): ASYNC. Sem reboot. | `GuiMenu.cpp:2770-2785`; `PKG/archr/sources/scripts/setrootpass` |
| Segurança (rede) | `system.security.enabled` — só no ramo BATOCERA, não compilado | | | |
| Backup | `backuptool backup` | Executado pelo ES como `/usr/bin/run "/usr/bin/backuptool backup"` (que, com `UI_SERVICE` contendo "sway", abre um terminal `foot -F` em tela cheia). Direto: `zip -9 -r /storage/roms/backup/ArchR_BACKUP.zip <lista>` (o arquivo anterior vira `ARCHIVED_ArchR_BACKUP-<aa-mm-dd_hh_mm_ss>.zip`). Lista padrão: `fancontrol.conf`, `backuptool.conf`, `/var/lib/bluetooth/*`, `/var/lib/connman*`, `system.cfg`, `ppsspp`, `retroarch`, `moonlight`, `game`, `/storage/.emulationstation/es_*.cfg`, scripts/themesettings/themes de `/storage/.emulationstation`. Personalizável por `/storage/.config/backuptool.conf` (`LOCATIONS=(...)`). **Várias entradas apontam para caminhos que não existem no ArchR** (ver lacunas). | ASYNC (zip de dezenas de MB); sem reboot. | `GuiMenu.cpp:256-262` |
| Restaurar | `backuptool restore` | `unzip -o /storage/roms/backup/ArchR_BACKUP.zip -d /`. Sobrescreve sem confirmação própria. **Não reinicia nada**; o ES avisa "WILL REBOOT" mas o script não reinicia. O launcher deve reiniciar o sistema depois. | ASYNC; reboot recomendado | `GuiMenu.cpp:263-269` |
| Reset do RetroArch | `factoryreset retroarch` / `retroarch-full` | Copia `/usr/config/retroarch` de volta. Sem reboot (o `retroarch-full` + `overlays` reinicia). | rápido | `GuiMenu.cpp:283-325` |
| Reset de overlays | `factoryreset overlays` | Apaga os `upperdir` dos `tmp-*.mount`, `sync`, **`systemctl reboot`** | reboot | |
| Reset mednafen / standalone / portmaster | `factoryreset mednafen` / `standalone` / `portmaster` | Restaura configs de `/usr/config`; `portmaster` reinstala o PortMaster do zip | `standalone`/`portmaster`: segundos; sem reboot | |
| Reset de áudio | `factoryreset audio` | Para PipeWire, apaga `/storage/.local/state`, `/storage/.config/pulse`, `/storage/asound*`, **reboot** | reboot | `GuiMenu.cpp:327` |
| Restauração de fábrica | `factoryreset ALL` | `swapoff`, desmonta `/storage/roms` e `/storage/games-external`, **apaga tudo em `/storage` exceto `/storage/.update*`, `/storage/roms*`, `/storage/games-*`**, recria `/storage/.config`, `sync`, **reboot**. ES alerta para ejetar o cartão de jogos antes. | longo (apaga muitos arquivos); ASYNC; reboot | `GuiMenu.cpp:334-340` |

O `run` do ES (`/usr/bin/run "<cmd>"`) com sway ativo executa o comando dentro de `foot -F` (terminal visível); não existe saída máquina-legível desses scripts. Para progresso o launcher deve executar os scripts diretamente e mostrar uma tela de "ocupado" própria.

---

## 13. Lacunas, defeitos e armadilhas de backend (consolidado)

Marcação: **[quebrado]** não funciona hoje; **[ausente]** não existe backend; **[armadilha]** funciona mas exige cuidado do launcher.

1. **[ausente] Wi-Fi: esquecer rede.** Nem `wifictl` nem ES. Implementar (`iwctl known-networks … forget` ou apagar perfis) e, se for a rede de `wifi.ssid`, limpar `wifi.ssid`/`wifi.key`.
2. **[ausente] Wi-Fi: lista sem segurança/sinal.** `iwd_get-networks` só imprime SSIDs. Para marcar aberta/WPA/WPA3/802.1X e conhecida é preciso estender o script (a D-Bus do iwd tem `Network.Type` = `open`/`psk`/`8021x`, `KnownNetwork`) ou chamar `iwctl station <dev> get-networks`.
3. **[ausente] Wi-Fi 802.1X sem UI/CLI**; só perfil `.8021x` escrito à mão. O launcher teria de gravá-lo (nome codificado conforme `encode_iwd_name`).
4. **[armadilha] `wifictl` sai 0 sem interface Wi-Fi** e não explica a falha de conexão (motivo só em `/var/log/wifictl.log`). Rede oculta: caminho existe, sem teste positivo.
5. **[armadilha] Reconexão no boot depende de `wifi.ssid`/`wifi.key` no cfg** (`099-networkservices`); gravar a rede usada, e cuidar de `wifi.adhoc.enabled=1` (sequestra o SSID).
6. **[quebrado] `archr-bluetooth` sempre sai 0**; sucesso/falha só pela saída ou `bluetoothctl info`. `trust` pode demorar 60 s mesmo falhando.
7. **[quebrado, a confirmar] "Parear controles automaticamente" (`trust input`)** não inicia discovery (nenhum `start` em `/run/bt_discovery_control`), só funciona se já houver varredura ativa.
8. **[armadilha] Bluetooth sem detecção de hardware**; a UI aparece em aparelhos sem rádio. Tipo do dispositivo só distingue `joystick`; fones aparecem sem tipo em `list`.
9. **[ausente] Rumble/vibração no RK3326**: sem `rumble_enable`; R36S força o pino em baixo no DTS. O item P0 não pode ser entregue sem trabalho de kernel/DTS (ou deve ficar oculto).
10. **[quebrado] Layout de teclado sem efeito**: `system.kblayout` não é lido por nada no Wayland/sway (`xkb_layout "us"` fixo; `setxkbmap` é X11). Aplicação possível: `swaymsg input type:keyboard xkb_layout <l>` e `xkb_variant`, mais reaplicação no boot.
11. **[armadilha] Hostname só aplica no boot** e sempre ganha sufixo `-xxxx`; sem validação de caracteres; default interno do ES é `BATOCERA` (só se a chave faltar).
12. **[armadilha] Idioma/fuso dependem de `es_settings`** (ExecStartPre do serviço antigo): o serviço do launcher deve executá-lo (ou equivalente) para gravar `/storage/.cache/timezone`, reiniciar `tz-data.service` e gerar o locale. A geração de locale pode travar o início por minutos na primeira troca. `archr-config tz` aplica fuso ao vivo mas não grava `system.timezone`.
13. **[quebrado] Atualização: canais.** UI antiga oferece `auto`/`release`(=`stable`)/`nightly`; o script só entende `stable|next|dev` (o resto vira `stable`). Seed do cfg: `updates.branch=auto`.
14. **[quebrado] Fluxo de atualização do ES** (ver seção 9): confirmação nunca aparece porque o ES espera uma única linha de saída em `archr-update check`. O launcher deve implementar o fluxo próprio (verificar, confirmar, executar, mostrar progresso).
15. **[armadilha] `archr-update check` usa exit 1 tanto para "sem atualização" quanto para "erro de rede/sync"**; diferenciar pelo texto da 1ª linha. Sem progresso percentual (`>>> NN%` nunca é emitido); progresso só indeterminado ou contando linhas `(n/m)` do pacman (inferido). O "APPLY UPDATE" do ES só reinicia a UI, não o sistema.
16. **[quebrado] Suspender no menu de energia do ES não existe** (`pm-suspend` ausente). O caminho do aparelho é `archr-fake-suspend`, que **depende da API HTTP do ES na porta 1234** (`/runningGame`, `/shutdown`). O launcher precisa servir equivalentes ou trocar o script para outro mecanismo, senão o desligamento pós-suspensão não salva nada e a detecção "jogo rodando" falha.
17. **[armadilha] `suspendmode`**: não gravar `system.suspendmode` antes de chamar o script (ele compara com o arquivo e não age se igual). A lista de modos do ES não inclui `off`, que é o efetivo do RK3326.
18. **[armadilha] Governador de CPU**: só 4 funções existem (`performance ondemand schedutil powersave`); `conservative`/`userspace`/`default` falham mudos. Filtrar a lista.
19. **[armadilha] Automount/merged**: o ES chama o restart com a chave ainda só em memória e com argumento espúrio; o launcher deve gravar primeiro e reiniciar sem argumento. O serviço desmonta `/storage/roms` e pode demorar (fsck).
20. **[armadilha] `runSystemCommand` do ES nunca devolve falha**; falhas de `device-switch`, `setrootpass`, `gpudriver` etc. passam despercebidas. O launcher deve checar o status real.
21. **[armadilha] `setrootpass` via shell concatenado** é inseguro/frágil no ES; usar argv. A senha fica em texto puro em `root.password` no cfg (seed `archr`).
22. **[quebrado] `backuptool` aponta para caminhos mortos**: `/storage/.emulationstation/es_*.cfg` (o ES do ArchR usa `/storage/.config/emulationstation`; **`es_input.cfg` e `es_settings.cfg` ficam fora do backup**), `/var/lib/connman*` (o Wi-Fi é iwd: `/var/lib/iwd` → `/storage/.cache/iwd` **não é salvo**). `/var/lib/bluetooth` e `system.cfg` estão cobertos. O restore não reinicia, apesar do aviso do ES. O ES também mostra o nome do arquivo com espaço (`ARCH R_BACKUP.zip`), mas o arquivo é `ArchR_BACKUP.zip` (`OS_NAME`).
23. **[armadilha] Brilho/volume/governadores com permissão root**: gravar em `/sys` exige root.
24. **[armadilha] Escrita concorrente em `system.cfg`**: o ES não usa a trava dos scripts; o launcher deve usar a trava `/tmp/.system.cfg.lock` e escrita atômica, e **recarregar o arquivo** antes de ler (scripts mudam chaves por conta própria: `set_setting` em `volume`, `brightness`, `turbomode`, `gpudriver`, `suspendmode`, `archr-bluetooth`).
25. **[armadilha] `std::to_string(float)`** do ES grava sliders como `50.000000`; consumidores em shell toleram, mas o launcher deve gravar inteiros.
26. **[armadilha] `runemu.sh` e `es_settings` ainda matam/gerenciam `emulationstation`** (`set_kill set "emulationstation"`, `UI_SERVICE="sway.service essway.service"`, `ES_CONFIG=/storage/.emulationstation/es_settings.cfg`). Revisar quando o serviço do launcher substituir `essway.service`/`emustation.service`.

---

## Apêndice A: dump de `es_features.cfg` (imagem atual)

Gerado a partir de `IMG/usr/config/emulationstation/es_features.cfg` (sem as linhas `vulkan`).
Formato: `EMU <emulador> features=<flags do atributo>`; `E <rótulo> | <value→sufixo da chave> | <choices (value)>` = opção do emulador;
`CORE <núcleo> features=<flags>`; `C ...` = opção do núcleo. A chave final é `<sistema>.<value>` ou `<sistema>["jogo"].<value>`.
Núcleos sem linhas `C` herdam só as opções por flag da seção 6.3 e as do emulador (`E`).

```
EMU mednafen features= None n= 2
   E shader | shader | none,autoip,autoipsharper,scale2x,sabr,ipsharper,ipxnoty,ipynotx,ipxnotysharper,ipynotxsharper,goat
   E stretch | stretch | 0,full,aspect,aspect_int
  CORE lynx features= None n= 0
  CORE gb features= None n= 1
     C system_type | system_type | auto,dmg,cgb,agb
  CORE gba features= None n= 1
     C tblur | tblur | 0,1
  CORE ngp features= None n= 0
  CORE nes features= None n= 3
     C correct_aspect | correct_aspect | 0,1
     C clipsides | clipsides | 0,1
     C no8lim | no8lim | 0,1
  CORE pce features= None n= 2
     C forcesgx | forcesgx | 0,1
     C nospritelimit | nospritelimit | 0,1
  CORE pce_fast features= None n= 5
     C correct_aspect | correct_aspect | 0,1
     C forcesgx | forcesgx | 0,1
     C nospritelimit | nospritelimit | 0,1
     C ocmultiplier | ocmultiplier | 1,2,3,4
     C cdspeed | cdspeed | 1,2,4,6,8,10
  CORE pcfx features= None n= 2
     C cpu_emulation | cpu_emulation | fast,accurate,auto
     C cdspeed | cdspeed | 2,4,6,8,10
  CORE gg features= None n= 0
  CORE md features= None n= 1
     C correct_aspect | correct_aspect | 0,1
  CORE sms features= None n= 0
  CORE snes features= None n= 0
  CORE snes_faust features= None n= 6
     C correct_aspect | correct_aspect | 0,1
     C spex | spex | 0,1
     C spex.sound | spex.sound | 0,1
     C superfx.clock_rate | superfx.clock_rate | 75,100,150,200,300,400,500
     C superfx.icache | superfx.icache | 0,1
     C cx4.clock_rate | cx4.clock_rate | 100,150,200,300,400,500
  CORE ss features= None n= 5
     C correct_aspect | correct_aspect | 0,1
     C input.port1 | input.port1 | gamepad,3dpad,gun
     C input.port1.3dpad.mode.defpos | input.port1.3dpad.mode.defpos | digital,analog
     C cart | cart | auto,none,backup,extram1,extram4,cs1ram16
     C cart.auto_default | cart.auto_default | none,backup,extram1,extram4,cs1ram16
  CORE vb features= None n= 2
     C cpu_emulation | cpu_emulation | fast,accurate
     C 3dmode | 3dmode | anaglyph,cscope,sidebyside,vli,hli,left,right
  CORE wsswan features= None n= 0
  CORE psx features= None n= 2
     C correct_aspect | correct_aspect | 0,1
     C input.port1 | input.port1 | gamepad,dualshock
EMU mupen64plus features= None n= 0
  CORE mupen64plus-sa features= None n= 9
     C video plugin | video_plugin | gliden64,gl64mk2,rice
     C game aspect ratio | game_aspect_ratio | 4:3,fullscreen
     C internal resolution | internal_resolution | 1,2,3,4
     C core plugin | core_plugin | mupen,simple
     C rsp plugin | rsp_plugin | hle,mle
     C input configuration | input_configuration | standard,custom
     C controller pak | controller_pak | 1,2,5
     C show fps | show_fps | true,false
     C gliden64 profiles | gliden64_profiles | balanced,performance
EMU touchhle features= None n= 0
  CORE touchhle-sa features= None n= 2
     C device type | device_type | iphone,ipad
     C upscale | upscale | 1,2,3,4
EMU yabasanshiro features= None n= 0
  CORE yabasanshiro-sa features= None n= 1
     C use hlebios | use_hlebios | 1,0
EMU ppsspp features= None n= 0
  CORE ppsspp-sa features= cheevos n= 6
     C auto frame skip | auto_frame_skip | 1,0
     C graphics backend | graphics_backend | opengl
     C internal resolution | internal_resolution | 1,2,3,4
     C show fps | show_fps | 1,0
     C skip buffer effects | skip_buffer_effects | 1,0
     C vsync | vsync | 1,0
EMU flycast features= None n= 0
  CORE flycast-sa features= cheevos n= 7
     C auto frame skipping | auto_frame_skipping | off,normal,max
     C aspect ratio | aspect_ratio | 4/3,w,sw
     C graphics backend | graphics_backend | opengl
     C internal resolution | internal_resolution | 0,1,2,3,4,5
     C show fps | show_fps | 1,0
     C vsync | vsync | 0,1
     C clock speed | clock_speed | 0,222,333
EMU duckstation features= None n= 0
  CORE duckstation-sa features= cheevos n= 5
     C aspect ratio | aspect_ratio | 0,1,2
     C graphics backend | graphics_backend | opengl,software
     C internal resolution | internal_resolution | 1,2,3,4,5
     C show fps | show_fps | true,false
     C vsync | vsync | off,on
EMU dolphin features= None n= 0
  CORE dolphin-qt-gc features= cheevos n= 23
     C anti aliasing | anti_aliasing | 0,2m,2s,4m,4s,8m,8s
     C aspect ratio | aspect_ratio | 0,1,2,3
     C audio backend | audio_backend | hle,lle
     C auto swap discs | auto_swap_discs | 1,0
     C clock speed | clock_speed | 050,075,100,125,150,200,300,400
     C enable cheats | enable_cheats | 1,0
     C gamecube controller profile | gamecube_controller_profile | south,east,custom
     C graphics backend | graphics_backend | opengl
     C internal resolution | internal_resolution | 1,2,3,4
     C rumble | rumble | true,false
     C save type | save_type | 1,8
     C save gci region | save_gci_region | usa,eur,jap
     C shader mode | shader_mode | 0,1,2,3
     C shader precompile | shader_precompile | true,false
     C show fps | show_fps | true,false
     C skip efb cpu access | skip_efb_cpu_access | true,false
     C store efb to texture only | store_efb_to_texture_only | true,false
     C store xfb to texture only | store_xfb_to_texture_only | true,false
     C texture cache accuracy | texture_cache_accuracy | 0,512,128
     C use bios | use_bios | false,true
     C vsync | vsync | 1,0
     C widescreen hack | widescreen_hack | true,false
     C write protect configs | write_protect_configs | true,false
  CORE dolphin-sa-gc features= None n= 24
     C anti aliasing | anti_aliasing | 0,2m,2s,4m,4s,8m,8s
     C aspect ratio | aspect_ratio | 0,1,2,3
     C audio backend | audio_backend | hle,lle
     C auto swap discs | auto_swap_discs | 1,0
     C clock speed | clock_speed | 050,075,100,125,150,200,300,400
     C enable cheats | enable_cheats | 1,0
     C gamecube controller profile | gamecube_controller_profile | south,west,custom
     C hotkey enable button | hotkey_enable_button | mode,select
     C graphics backend | graphics_backend | opengl
     C rumble | rumble | true,false
     C internal resolution | internal_resolution | 1,2,4,6
     C save type | save_type | 1,8
     C save gci region | save_gci_region | usa,eur,ja[
     C shader mode | shader_mode | 0,1,2,3
     C shader precompile | shader_precompile | true,false
     C show fps | show_fps | true,false
     C skip efb cpu access | skip_efb_cpu_access | true,false
     C store efb to texture only | store_efb_to_texture_only | true,false
     C store xfb to texture only | store_xfb_to_texture_only | true,false
     C texture cache accuracy | texture_cache_accuracy | 0,512,128
     C use bios | use_bios | false,true
     C vsync | vsync | 1,0
     C widescreen hack | widescreen_hack | true,false
     C write protect configs | write_protect_configs | true,false
  CORE dolphin-qt-wii features= cheevos n= 19
     C anti aliasing | anti_aliasing | 0,2m,2s,4m,4s,8m,8s
     C aspect ratio | aspect_ratio | 0,1,2,3
     C clock speed | clock_speed | 050,075,100,125,150,200,300,400
     C enable cheats | enable_cheats | 1,0
     C graphics backend | graphics_backend | opengl
     C internal resolution | internal_resolution | 1,2,3,4
     C rumble | rumble | true,false
     C shader mode | shader_mode | 0,1,2,3
     C shader precompile | shader_precompile | true,false
     C show fps | show_fps | true,false
     C wii controller profile | wii_controller_profile | vremote,hremote,nunchuck,classic,custom
     C hotkey enable button | hotkey_enable_button | mode,select
     C skip efb cpu access | skip_efb_cpu_access | true,false
     C store efb to texture only | store_efb_to_texture_only | true,false
     C store xfb to texture only | store_xfb_to_texture_only | true,false
     C texture cache accuracy | texture_cache_accuracy | 0,512,128
     C vsync | vsync | 1,0
     C widescreen hack | widescreen_hack | true,false
     C write protect configs | write_protect_configs | true,false
  CORE dolphin-sa-wii features= None n= 19
     C anti aliasing | anti_aliasing | 0,2m,2s,4m,4s,8m,8s
     C aspect ratio | aspect_ratio | 0,1,2,3
     C clock speed | clock_speed | 050,075,100,125,150,200,300,400
     C enable cheats | enable_cheats | 1,0
     C graphics backend | graphics_backend | opengl
     C internal resolution | internal_resolution | 1,2,4,6
     C rumble | rumble | true,false
     C shader mode | shader_mode | 0,1,2,3
     C shader precompile | shader_precompile | true,false
     C show fps | show_fps | true,false
     C wii controller profile | wii_controller_profile | vremote,hremote,nunchuck,classic,custom
     C hotkey enable button | hotkey_enable_button | mode,select
     C skip efb cpu access | skip_efb_cpu_access | true,false
     C store efb to texture only | store_efb_to_texture_only | true,false
     C store xfb to texture only | store_xfb_to_texture_only | true,false
     C texture cache accuracy | texture_cache_accuracy | 0,512,128
     C vsync | vsync | 1,0
     C widescreen hack | widescreen_hack | true,false
     C write protect configs | write_protect_configs | true,false
EMU cemu features= None n= 0
  CORE cemu-sa features= None n= 27
     C wiiu controller profile | wiiu_controller_profile | Wii U GamePad,Wii U Pro Controller,Wii U Classic Controller,Wiimote
     C wiiu controller layout | wiiu_controller_layout | nintendo,xbox
     C gamepad enabled | gamepad_enabled | false,true
     C online enabled | online_enabled | false,true
     C graphics backend | graphics_backend | opengl
     C gdk backend | gdk_backend | wayland,x11
     C vsync | vsync | 0,1,2
     C gx2 draw done sync | gx2_draw_done_sync | true,false
     C upscale filter | upscale_filter | 0,1,2,3
     C downscale filter | downscale_filter | 0,1,2,3
     C fullscreen scaling | fullscreen_scaling | 0,1
     C async shader compile | async_shader_compile | true,false
     C vk accurate barriers | vk_accurate_barriers | true,false
     C overlay position | overlay_position | 0,1,2,3,4,5,6
     C overlay text scale | overlay_text_scale | 100,150,200,250,300
     C overlay text color | overlay_text_color | 000000,FFFFFF,0000FF,00FF00,FF0000,00FFFF,FF00FF,FFFF00
     C overlay text transparency | overlay_text_transparency | FF,EF,DF,CF,BF,AF,9F,8F,7F,6F,5F
     C overlay show fps | overlay_show_fps | true,false
     C overlay draw calls | overlay_draw_calls | true,false
     C overlay show cpu usage | overlay_show_cpu_usage | true,false
     C overlay show cpu per core usage | overlay_show_cpu_per_core_usage | true,false
     C overlay show ram usage | overlay_show_ram_usage | true,false
     C overlay show vram usage | overlay_show_vram_usage | true,false
     C notification position | notification_position | 0,1,2,3,4,5,6
     C notification text scale | notification_text_scale | 100,150,200,250,300
     C notification text color | notification_text_color | 000000,FFFFFF,0000FF,00FF00,FF0000,00FFFF,FF00FF,FFFF00
     C notification text transparency | notification_text_transparency | FF,EF,DF,CF,BF,AF,9F,8F,7F,6F,5F
EMU azahar features= None n= 0
  CORE azahar-sa features= None n= 10
     C cpu speed | cpu_speed | 0,1,2,3,4,5
     C emulate mouse | emulate_mouse | 1,0
     C graphics backend | graphics_backend | 1
     C cache shaders | cache_shaders | 1,0
     C hardware shaders | hardware_shaders | 1,0
     C accurate hardware shaders | accurate_hardware_shaders | 1,0
     C resolution scale | resolution_scale | 1,2,3,0
     C disable right eye render | disable_right_eye_render | 1,0
     C rotate screen | rotate_screen | 1,0
     C screen layout | screen_layout | 0,1a,1b,2,3,4,5
EMU melonds features= None n= 0
  CORE melonds-sa features= cheevos n= 9
     C console type | console_type | 0,1
     C direct boot | direct_boot | 1,0
     C graphics backend | graphics_backend | 0,1,2
     C internal resolution | internal_resolution | 1,2,3,4,5
     C screen orientation | screen_orientation | 1,2,3
     C screen layout | screen_layout | 0,1,2,4,5,6
     C screen swap | screen_swap | 1,0
     C screen rotation | screen_rotation | 0,1,2,3
     C vsync | vsync | 1,0
EMU aethersx2 features= None n= 0
  CORE aethersx2-sa features= cheevos n= 9
     C aspect ratio | aspect_ratio | 0,1,2
     C bilinear filtering | bilinear_filtering | 0,1,2,3
     C ee cycle rate | ee_cycle_rate | 0,1,2,3,4,5,6
     C ee cycle skip | ee_cycle_skip | 0,1,2,3
     C hw download mode | hw_download_mode | 0,1,2,3
     C graphics backend | graphics_backend | 0,1,3
     C internal resolution | internal_resolution | 1,1.25,1.5,1.75,2,2.25,2.5,2.75,3
     C show fps | show_fps | true,false
     C enable widescreen patches | enable_widescreen_patches | true,false
EMU rpcs3 features= None n= 0
  CORE rpcs3-sa features= None n= 14
     C aspect ratio | aspect_ratio | 4x3,16x9
     C async texture streaming | async_texture_streaming | true,false
     C frame limit | frame_limit | 30,60,auto
     C internal ps3 resolution | internal_ps3_resolution | 480,576,720,1080,native
     C internal resolution scale | internal_resolution_scale | 25,50,75,100
     C graphics backend | graphics_backend | opengl
     C multithreaded rsx | multithreaded_rsx | true,false
     C shader precision | shader_precision | low,high,ultra
     C spu xfloat accuracy | spu_xfloat_accuracy | approximate,accurate,relaxed
     C write color buffers | write_color_buffers | true,false
     C zcull accuracy | zcull_accuracy | relaxed,approximate,precise
     C vsync | vsync | true,false
     C performance overlay | performance_overlay | true,false
     C start ui | start_ui | true,false
EMU xemu features= None n= 0
  CORE xemu-sa features= None n= 10
     C aspect ratio | aspect_ratio | 0,1,2
     C fit | fit | stretch,center,scale,integer
     C cache shaders to disk | cache_shaders_to_disk | true,false
     C cpu clock speed | cpu_clock_speed | 050,060,070,080,090,100,110,120,130,140,150
     C graphics backend | graphics_backend | opengl
     C internal resolution | internal_resolution | 1,2,3,4
     C show fps | show_fps | 1,0
     C skip boot animation | skip_boot_animation | true,false
     C system memory | system_memory | 64,128
     C vsync | vsync | true,false
EMU portmaster features= None n= 0
  CORE portmaster features= None n= 1
     C controller layout | controller_layout | xbox,nintendo,custom
EMU gzdoom features= None n= 0
  CORE gzdoom-sa features= None n= 1
     C vsync | vsync | true,false
EMU wine features= None n= 0
  CORE wine features= None n= 1
     C controller layout | controller_layout | xbox,nintendo,custom
EMU moonlight features= None n= 0
  CORE moonlight features= None n= 1
     C controller layout | controller_layout | xbox,xbox_swap_shoulders_triggers,nintendo
EMU pico-8 features= pixel_perfect n= 0
EMU skyemu features= None n= 0
  CORE skyemu-sa features= cheevos n= 0
EMU retroarch features= decoration, ratio, smooth, shaders, pixel_perfect, latency_reduction, game_translation n= 0
  CORE 2048 features= netplay, rewind, autosave n= 0
  CORE 81 features= netplay, rewind, autosave n= 0
  CORE a5200 features= netplay, rewind, autosave n= 0
  CORE arduous features= autosave n= 0
  CORE atari800 features= netplay, rewind, autosave n= 0
  CORE bluemsx features= netplay, rewind, autosave, cheevos n= 0
  CORE bsnes features= autosave n= 0
  CORE bsnes_hd_beta features= autosave n= 0
  CORE bsnes_mercury_accuracy features=  autosave n= 0
  CORE bsnes_mercury_balanced features=  autosave n= 0
  CORE bsnes_mercury_performance features=  autosave n= 0
  CORE cannonball features= netplay, rewind, autosave n= 0
  CORE cap32 features= netplay, rewind, autosave n= 0
  CORE crocods features= netplay, rewind, autosave n= 0
  CORE daphne features= netplay, rewind, autosave n= 0
  CORE desmume features= netplay, rewind, autosave n= 0
  CORE dolphin features= netplay, rewind, autosave n= 0
  CORE DoubleCherryGB features= decoration, netplay, rewind, autosave n= 0
  CORE dosbox_pure features= netplay, rewind, autosave n= 0
  CORE dosbox_core features= netplay, rewind, autosave n= 0
  CORE easyrpg features= netplay, rewind, autosave n= 0
  CORE emuscv features= netplay, rewind, autosave n= 0
  CORE fake08 features= autosave n= 0
  CORE fbalpha2012 features= netplay, rewind, autosave n= 0
  CORE fbneo features= netplay, rewind, autosave, cheevos n= 0
  CORE fceumm features= netplay, rewind, autosave, cheevos n= 0
  CORE flycast features= netplay, rewind, autosave n= 4
     C netplay | netplay | 
     C rewind | rewind | 
     C autosave | autosave | 
     C frame skip | frame_skip | disabled,some,more
  CORE flycast2021 features= netplay, rewind, autosave n= 0
  CORE freeintv features= netplay, rewind, autosave, cheevos n= 0
  CORE fuse features= netplay, rewind, autosave n= 0
  CORE gambatte features= decoration, colorization, netplay, rewind, autosave, cheevos n= 0
  CORE gearboy features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE gearcoleco features= netplay, rewind, autosave, cheevos n= 0
  CORE gearsystem features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE genesis_plus_gx features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE genesis_plus_gx_wide features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE geolith features= netplay, rewind, autosave n= 0
  CORE gme features= netplay, rewind, autosave n= 0
  CORE gpsp features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE gw features= netplay, rewind, autosave n= 0
  CORE handy features= netplay, rewind, autosave, cheevos n= 0
  CORE hatari features= netplay, rewind, autosave n= 0
  CORE jaxe features= netplay, rewind, autosave n= 0
  CORE mame2003_plus features= netplay, rewind, autosave n= 4
     C netplay | netplay | 
     C rewind | rewind | 
     C autosave | autosave | 
     C tatemode | tatemode | 1,0
  CORE mame2010 features= netplay, rewind, autosave n= 0
  CORE mame2016 features= netplay, rewind, autosave n= 0
  CORE melonds features= netplay, rewind, autosave, cheevos n= 0
  CORE melondsds features= netplay, rewind, autosave, cheevos n= 0
  CORE play features= netplay, rewind, autosave n= 0
  CORE beetle_gba features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE beetle_lynx features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_ngp features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE beetle_pce features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_pce_fast features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_pcfx features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_psx features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_saturn features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_supafaust features= netplay, rewind, autosave n= 0
  CORE beetle_supergrafx features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_vb features= netplay, rewind, autosave, cheevos n= 0
  CORE beetle_wswan features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE mesen-s features= netplay, rewind, autosave, cheevos n= 0
  CORE mesen features= netplay, rewind, autosave, cheevos n= 0
  CORE mgba features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE mojozork features= autosave n= 0
  CORE mrboom features= netplay, rewind, autosave n= 0
  CORE mu features= netplay, rewind, autosave n= 0
  CORE mupen64plus features= netplay, rewind, cheevos n= 0
  CORE mupen64plus_next features= netplay, rewind, autosave, cheevos n= 0
  CORE nekop2 features= netplay, rewind, autosave n= 0
  CORE neocd features= netplay, rewind, autosave n= 0
  CORE nestopiaCV features= netplay, rewind, autosave n= 0
  CORE nestopia features= netplay, rewind, autosave, cheevos n= 0
  CORE np2kai features= netplay, rewind, autosave n= 0
  CORE o2em features= netplay, rewind, autosave, cheevos n= 0
  CORE opera features= netplay, rewind, autosave, cheevos n= 0
  CORE panda3ds features= netplay, rewind, cheevos n= 0
  CORE parallel_n64 features= netplay, rewind, autosave, cheevos n= 9
     C netplay | netplay | 
     C rewind | rewind | 
     C autosave | autosave | 
     C cheevos | cheevos | 
     C parallel n64 video core | parallel_n64_video_core | glide64,rice,gln64,angrylion
     C parallel n64 internal resolution | parallel_n64_internal_resolution | 320x240,640x480,960x720,1440x1080
     C parallel n64 gamespeed | parallel_n64_gamespeed | original,fullspeed
     C parallel n64 gfx accuracy | parallel_n64_gfx_accuracy | low,medium,high,veryhigh
     C parallel n64 controller pak | parallel_n64_controller_pak | none,memory,rumble
  CORE pcsx_rearmed features= netplay, rewind, autosave, cheevos n= 0
  CORE pcsx_rearmed32 features= netplay, rewind, autosave, cheevos n= 0
  CORE play features= autosave n= 0
  CORE picodrive features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE pokemini features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE ppsspp features= netplay, rewind, autosave, cheevos n= 0
  CORE prboom features= netplay, rewind, autosave n= 0
  CORE prosystem features= netplay, rewind, autosave, cheevos n= 0
  CORE puae features= netplay, rewind, autosave n= 0
  CORE puae2021 features= netplay, rewind, autosave n= 0
  CORE px68k features= netplay, rewind, autosave n= 0
  CORE potator features= decoration, cheevos n= 0
  CORE quasi88 features= netplay, rewind, autosave, cheevos n= 0
  CORE quicknes features= netplay, rewind, autosave, cheevos n= 0
  CORE race features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE same_cdi features= netplay, rewind, autosave, cheevos n= 0
  CORE sameboy features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE sameduck features= netplay, rewind, autosave n= 0
  CORE scummvm features= netplay, rewind, autosave n= 0
  CORE skyemu features= autosave, cheevos n= 0
  CORE smsplus features= decoration, rewind, autosave, cheevos n= 0
  CORE snes9x2002 features= netplay, rewind, autosave, cheevos n= 0
  CORE snes9x2005_plus features= netplay, rewind, autosave, cheevos n= 0
  CORE snes9x2010 features= netplay, rewind, autosave, cheevos n= 0
  CORE snes9x features= netplay, rewind, autosave, cheevos n= 0
  CORE stella features= netplay, rewind, autosave, cheevos n= 0
  CORE swanstation features= autosave n= 0
  CORE tgbdual features= decoration, netplay, rewind, autosave n= 0
  CORE theodore features= rewind, autosave n= 0
  CORE tyrquake features= netplay, rewind, autosave n= 0
  CORE uae4arm features= netplay, rewind, autosave n= 0
  CORE uzem features= netplay, rewind, autosave n= 0
  CORE vba_next features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE vbam features= decoration, netplay, rewind, autosave, cheevos n= 0
  CORE vecx features= netplay, rewind, autosave, cheevos n= 0
  CORE vice_x128 features= netplay, rewind, autosave n= 0
  CORE vice_x64 features= netplay, rewind, autosave n= 0
  CORE vice_xplus4 features= netplay, rewind, autosave n= 0
  CORE vice_xvic features= netplay, rewind, autosave n= 0
  CORE vice_xpet features= netplay, rewind, autosave n= 0
  CORE vircon32 features= autosave n= 0
  CORE virtualjaguar features= netplay, cheevos n= 0
  CORE x1 features= netplay, rewind, autosave n= 0
  CORE yabasanshiro features= netplay, rewind, autosave n= 0
  CORE kronos features= netplay, rewind, autosave n= 6
     C force hle bios | force_hle_bios | enabled,disabled
     C addon cartridge | addon_cartridge | none,1M_extended_ram,4M_extended_ram,16M_extended_ram,512K_backup_ram,1M_backup_ram,2M_backup_ram,4M_backup_ram
     C tesselation | tesselation | perspective_correction,gpu_tesselation,cpu_tesselation
     C resolution | resolution | original,480p,720p,1080p
     C compute shader | compute_shader | disabled,enabled
     C transparency | transparency | disabled,enabled
EMU supermodel features= None n= 0
  CORE supermodel-sa features= None n= 3
     C resolution | resolution | 0,1,2
     C vsync | vsync | true,false
     C rendering engine | rendering_engine | 0,1
EMU drastic features= None n= 0
  CORE drastic-sa features= None n= 4
     C hires 3d | hires_3d | 1,0
     C threaded 3d | threaded_3d | 1,0
     C follow 3d renderer | follow_3d_renderer | 1,0
     C microphone sensitivity | microphone_sensitivity | 0,0.03,0.15,0.3
```
