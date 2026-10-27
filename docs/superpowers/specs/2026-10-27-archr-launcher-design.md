# archr-launcher: especificação de desenho

Data: 2026-10-27. Estado: aprovado em conversa, aguardando revisão do documento.

## 1. Objetivo

Um launcher de jogos próprio do ArchR para substituir o EmulationStation (ES) nos aparelhos RK3326: limpo, rápido de abrir e leve na memória, mas preparado para personalização futura (capas, música, temas, vídeos de prévia).

O ES leva cerca de 6 s para mostrar o menu no R36S (Cortex-A35, 1 GB de RAM), porque carrega todos os ~140 sistemas de uma vez: varre as pastas de ROMs, lê os `gamelist.xml`, analisa o tema de cada sistema e inicializa VLC e a base de nomes do MAME. O launcher novo lê só o que está na tela.

### Metas mensuráveis

- Primeira tela em **menos de 0,3 s** depois de o processo começar, com índice em cache, medida no R36S com a sonda de boot (`swaymsg -t get_tree` a cada 100 ms).
- Voltar de um jogo para a lista em menos de 0,3 s.
- Sem memória ocupada pelo launcher durante o jogo.

### Fora do escopo da versão 1

- Telas de configuração do sistema (Wi-Fi, Bluetooth, GPU, áudio etc.): ficam num app separado, `archr-settings`, que reaproveita as mesmas bibliotecas. O launcher só o abre.
- Scraping de capas: o launcher lê as mídias que o ES ou scrapers externos já baixaram.
- Vídeos de prévia: o formato do índice os prevê, mas a reprodução fica desligada na versão 1.
- Camada de controle durante o jogo (confirmação de saída, FPS, salvar/carregar estado): componente próprio, posterior, que se encaixa no supervisor (seção 4).
- Temas programáveis (Lua): possível depois, sobre os temas declarativos, sem mudar a arquitetura.

## 2. Decisões tomadas

| Decisão | Escolha | Motivo |
|---|---|---|
| Escopo | Só jogos, configurações em app separado | Launcher pequeno e rápido; mesmo kit visual nos dois |
| Telas | 640x480 primeiro, medidas em unidades relativas desde o início | Entrega rápido no aparelho principal sem refazer layout depois |
| Navegação | Listas de texto: sistemas, depois jogos; capa ao lado quando existir | Mais rápida e legível; um tema pode trocar a tela de sistemas por um carrossel |
| Linguagem | Rust | Desempenho de C, sem coletor de lixo, com segurança de memória para um programa que vai crescer |
| Gráficos | SDL2 (com SDL2_image e SDL2_ttf), via OpenGL ES | Já está na build e é o que todos os emuladores usam; fala Wayland (sway) e KMS. Isolado numa camada própria para permitir trocar por SDL3 no futuro |
| Temas | Declarativos em TOML, sem código executável | Um tema ruim não deixa o launcher lento nem o derruba |

## 3. Componentes

```
archr-launcher/
  crates/
    archr-ui/        desenho, fontes, temas, entrada (compartilhado com archr-settings)
    archr-config/    leitura e gravação do system.cfg com trava
    archr-index/     índice de sistemas e jogos, leitura de gamelist.xml
    archr-launcher/  o binário: telas, navegação, menu rápido
  session/           supervisor de sessão (shell) e unidade systemd
  themes/default/    tema padrão (também embutido no binário)
  docs/
```

O empacotamento na distro fica em `distribution/projects/ArchR/packages/ui/archr-launcher/package.mk`, usando o pacote `rust`/`cargo` que a build já tem.

## 4. Sessão: launcher, supervisor e jogo

Modelo de processos separados (referência de arquitetura: NextUI/MinUI; nenhum código copiado, a licença deles não permite):

1. `archr-launcher-session.service` roda o supervisor `archr-launcher-session`.
2. O supervisor executa o launcher e espera ele terminar.
3. Ao escolher um jogo, o launcher grava o comando em `/run/archr-launcher/next` e encerra com código 0.
4. O supervisor lê o arquivo, apaga-o e executa o comando, que é sempre o contrato atual do ES:
   `/usr/bin/runemu.sh <ROM> -P<sistema> --core=<núcleo> --emulator=<emulador> --controllers="<config>"`
5. Ao fim do jogo, o supervisor guarda o código de saída em `/run/archr-launcher/last-exit` e volta ao passo 2. O launcher, ao abrir, mostra uma mensagem curta se o jogo falhou, com o caminho do log.
6. Para desligar, reiniciar ou abrir o `archr-settings`, o launcher grava a ação no mesmo arquivo `next`, com um prefixo que a distingue de um jogo, e o supervisor a executa.

O `runemu.sh` não muda na versão 1: ele já aplica as configurações por jogo, pausa serviços e restaura tudo depois.

O serviço depende do sway, com a mesma espera por socket Wayland e saída ativa que o `start_es.sh` usa hoje (`swaymsg -t get_outputs` com `"active": true`).

## 5. Dados

### Fonte da lista de sistemas

O `es_systems.cfg` gerado na build (`/usr/config/emulationstation/es_systems.cfg`) continua sendo a fonte de sistemas, extensões, emuladores e núcleos padrão. O launcher só lê; nada muda na geração.

### Índice

Em `/storage/.cache/archr-launcher/`, formato binário compacto e versionado (descartado e refeito se a versão não bater ou se estiver corrompido):

- **Índice geral:** por sistema, quantidade de jogos e a data de modificação da pasta de ROMs. Sistemas vazios ficam escondidos.
- **Índice por sistema:** para cada jogo, caminho, nome limpo (sem `(USA)`, `[!]` etc.), caminhos de mídia e dados do `gamelist.xml` usados na tela.
- Refeito em segundo plano quando a data de modificação da pasta muda; a tela usa o índice antigo até o novo ficar pronto.

### Mídias

Mesma convenção do ES, para que as mídias que os usuários já têm apareçam sem migração:

1. Caminhos do `gamelist.xml` do sistema (`<image>`, `<thumbnail>`, `<video>` e demais), lidos só ao montar o índice.
2. Sem `gamelist.xml` ou sem a entrada: procura pelo nome da ROM em `<sistema>/media/<tipo>/` e depois em `<sistema>/images/`.

As capas são carregadas sob demanda, em segundo plano, só para o item selecionado e os vizinhos, já redimensionadas, num cache de miniaturas em `/storage/.cache/archr-launcher/thumbs/`. Sem capa, nada é carregado.

### Favoritos, recentes e histórico

Arquivo próprio do launcher em `/storage/.config/archr-launcher/`, gravado sem bloquear a tela. Na versão 1, importa uma vez os favoritos e a última vez jogada do `gamelist.xml`, quando existirem.

### system.cfg

Leitura e gravação pela crate `archr-config`, com a mesma trava dos scripts (`/tmp/.system.cfg.lock`, ver `wait_lock` em `distribution/projects/ArchR/packages/archr/profile.d/001-functions`), gravando num temporário e trocando com `rename`. O ES grava hoje sem a trava; o launcher não repete isso. Chaves usadas na versão 1: `<sistema>.emulator`, `<sistema>.core`, `<sistema>["<rom>"].emulator` e `<sistema>["<rom>"].core`.

## 6. Interface

### Telas

- **Sistemas:** coleções no topo (Favoritos, Recentes), depois os sistemas com jogos.
- **Jogos:** lista de nomes; capa ao lado quando existir.
- **Detalhes (opcional):** capa grande e dados do `gamelist.xml`.
- **Menu rápido:** emulador e núcleo do jogo, abrir configurações, desligar, reiniciar, suspender.

### Botões

Mesmo padrão do ES: A abre, B volta, X favorita, Y abre o menu rápido, L/R pulam página. Start e Select ficam reservados para coleções e para a camada de controle futura. O controle é lido pelo SDL2 com o mapeamento do `es_input.cfg`, para funcionar desde o primeiro boot.

### Música

Pasta de músicas definida pelo tema ou pelo usuário. Começa depois que a interface está desenhada. Sem música, o módulo de áudio nem é iniciado.

## 7. Temas

- Padrão em `/usr/share/archr-launcher/themes/`; do usuário em `/storage/.config/archr-launcher/themes/`.
- Um tema é uma pasta com `theme.toml` e recursos opcionais (fontes, imagens, música).
- O `theme.toml` define cores, fonte, tamanhos, margens e posições em unidades relativas à tela, imagens opcionais (fundo, logotipos de sistemas) e qual tela de sistemas usar (lista; carrossel fica previsto).
- O tema padrão não usa imagens. Um tema mais rico custa só o que usa.
- Tema ausente ou inválido: o launcher registra o erro e usa o tema padrão embutido no binário.

## 8. Falhas

| Situação | Comportamento |
|---|---|
| Jogo não abre | Supervisor guarda o código de saída; launcher mostra mensagem curta com o caminho do log |
| Tema quebrado | Tema padrão embutido |
| Índice corrompido ou de versão antiga | Descartado e refeito |
| `es_systems.cfg` ausente | Mensagem clara na tela e acesso ao menu rápido |
| Gravação do `system.cfg` | Trava + temporário + `rename`; nunca deixa o arquivo pela metade |
| Launcher trava ou cai | O serviço systemd o reinicia; o supervisor não depende do estado dele |

## 9. Testes

- **No PC:** o mesmo binário em janela de 640x480, com uma pasta de ROMs de exemplo e um `es_systems.cfg` de exemplo.
- **Automáticos:** índice (montagem, nomes limpos, leitura de `gamelist.xml`, mídias), temas (leitura, valores inválidos, queda para o padrão) e `system.cfg` (leitura com prioridade jogo > sistema > global, gravação com trava).
- **No aparelho:** sonda de tempo até a primeira tela; abrir e voltar de jogos de pelo menos três sistemas (um RetroArch, o N64 standalone e um port); trocar de emulador pelo menu rápido e conferir o `system.cfg`.

## 10. Transição

O launcher entra na imagem ao lado do ES, como serviço alternativo. O ES continua sendo o padrão até o launcher cobrir o necessário; a troca é uma configuração (`UI_SERVICE` no quirk `090-ui_service`). O `archr-settings` é um projeto seguinte, com especificação própria.
