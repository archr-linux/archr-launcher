# archr-launcher: refatoração de simplificação e robustez

Origem: auditoria de 2026-10-28 (HEAD 20ae12e) pedida pelo Douglas: "nível industrial, código o mais inteligente possível, nada de toneladas de código pra coisas simples". Meta: de ~12,3 mil para ~10 mil linhas Rust sem perder cobertura, com as fragilidades abaixo corrigidas. Critério: menos representações duplicadas, estados paralelos e caminhos repetidos; limites consistentes; contratos verificáveis. Não é reescrita.

## Fragilidades a corrigir

1. Leituras limitadas não são uniformes: system.cfg e arquivo de trava lidos sem limite (archr-config lib.rs:20); es_systems.cfg e gamelist.xml sem limite (systems.rs:51, gamelist.rs:65); cache e es_input.cfg abrem antes de checar o tipo (FIFO bloqueia; cache.rs:170, controllers.rs:45); `.size` das miniaturas sem limite (covers.rs:47). Corrigir com um único auxiliar: abertura `O_NONBLOCK`, checagem do descritor, limite real; XML acima do limite é rejeitado, nunca truncado. Symlinks de ROM e mídia continuam permitidos.
2. Trava compartilhada com o shell: `wait_lock` (distribution, profile.d/001-functions:80) deixa um trap de EXIT que apaga a trava de outro dono depois de liberar a sua. Corrige-se no helper shell (distribution), com teste multiprocesso. Trava vazia com mais de 2 s não prova abandono: tratar conteúdo vazio/inválido por tempo limite.
3. Store grava sem o limite de 256 KiB que usa na leitura; favoritos sem limite; store inválido vira vazio e é sobrescrito. Limite simétrico, erro visível, arquivo antigo preservado.
4. Cache por sistema validado só pelo maior mtime: não percebe troca de extensões nem de pasta. Identidade = caminho + extensões + carimbos individuais.
5. SIGTERM vira `Event::Quit` no SDL e é ignorado em tela cheia (main.rs:191): por isso parar o serviço esperava 4 s. Aceitar Quit em qualquer modo, sem publicar `next`; supervisor com orçamento curto quando só o launcher está aberto (os 4 s ficam para limpeza de jogo).
6. Desligar/Reiniciar (teste do Douglas): o supervisor chama `systemctl poweroff`, que volta na hora, e reabre o launcher. Decisão: o próprio launcher mostra o aviso, chama `systemctl poweroff|reboot` e fica no aviso até o sistema encerrar; o supervisor deixa de tratar essas ações.
7. Suspender: R36S tem `system.suspendmode=off`, systemd recusa. Ação vai para `/usr/bin/archr-fake-suspend power`; e o `archr-fake-suspend` (distribution:17) hoje trata chave ausente diferente de `off` — corrigir lá também.
8. Chaves de favoritos/recentes com caminho absoluto (`n64//storage/roms/...`): passar a `sistema/caminho-relativo-à-pasta-do-sistema`, com migração idempotente das antigas.
9. Atualizar coleções percorre todos os jogos e clona listas (library.rs:321): conjunto de membros e atualização localizada.
10. Item Configurações sem destino: esconder até as telas de configuração existirem.
11. Rename atômico sem sincronizar o diretório: sincronizar o diretório em config e store (cache e /run não precisam).

## Simplificação por módulo (metas)

theme.rs 499→280 (eliminar `RawTheme` duplicado; aplicar a tabela TOML direto sobre os padrões); render.rs 1217→850 (contexto de quadro, auxiliares concretos para rótulo/painel/linha; sem árvore genérica de widgets); text.rs 486→360 (fonte por face física); primitives.rs 439→300; badges.rs 139→90 (Grafite pré-renderizado, geração só para temas personalizados); status.rs 194→145; controllers.rs 175→135; library.rs 421→340; main.rs 337→260 (uma passagem para efeitos e transições); archr-config lib.rs 501→390; file_io.rs some (vai para um módulo comum de E/S com abertura limitada, leitura UTF-8 e publicação atômica; sem traits nem backends). Testes: fixtures compartilhadas e casos em tabela; preservar concorrência real, quoting pelo shell, symlinks, limites e fallback por campo.

## Abertura < 300 ms

Medido no R36S: vídeo 25, janela 72, contexto GL ~200, fontes ~50, badges+sombra ~20, desenho 12, apresentação ~45 = 366-400 ms. SDL 2.32 no Wayland não tem framebuffer shm nativo: o renderizador software também passa pelo EGL. Plano A (por medição A/B, 30 aberturas por variante): criar a janela GLES certa de saída (evita recriar janela, 20-70 ms), GLES2 explícito (0-40), reaproveitar a face TTF trocando tamanho (10-25), badges Grafite pré-renderizados (8-20), sombra pré-calculada (2-6), sem SDL_image na abertura (0-3), investigar a primeira apresentação (0-25). Orçamento-alvo: 265 ms. Plano B, só se A não bastar: backend shm de verdade (economia estimada 150-220 ms), como experimento separado.

## Passos (cada um testável sozinho, menor risco primeiro)

1. Fixtures consolidadas e testes de contrato para os itens acima (sem mudar produção).
2. Integração com a plataforma: Quit em tela cheia, parada curta sem jogo, desligar/reiniciar no launcher, suspender, Configurações escondido.
3. E/S unificada e persistência endurecida (itens 1, 3, 11); trava shell corrigida na distribution (item 2).
4. Identidade e invalidação (itens 4, 8, 9).
5. Tema e apresentação simplificados (mesmas capturas antes/depois).
6. Abertura por medição (plano A; B só se preciso).
