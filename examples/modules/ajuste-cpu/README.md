# Ajuste automático de CPU

Módulo oficial do archr-launcher (API 3, bash + jq, daemon). Aprende, jogo a
jogo, o menor teto de frequência da CPU em que o emulador não chega ao limite
da CPU, e economiza bateria com ele. É uma heurística de ocupação da CPU, não
uma contagem de quadros. Só a CPU e só naquela abertura: nunca a GPU, a
memória ou o seu ajuste de governador.

Como aprende:

- antes de cada jogo responde ao `launch.before` com o teto que está testando
  (`cpu_max_khz`, um dos `cpu.opps` do `initialize`);
- na volta, o `game.ended` traz a medida do `archr-perf-probe` (ocupação da
  thread mais ocupada do emulador, segundos no limite, corrente da bateria);
- folga clara (p95 abaixo de 75 % e nenhum segundo no limite): desce um
  degrau; no limite (p95 de 92 % ou 2 s ou mais no limite): sobe um e marca
  o piso; três sessões estáveis no mesmo teto: "aprendido". Um jogo
  aprendido que chegar ao limite volta um degrau;
- sessões com menos de dois minutos, ou ociosas na maior parte do tempo
  (pausa, menu), não contam; N64, PSP, Dreamcast e Saturn, e jogos que já
  chegam ao limite no máximo, ficam fora;
- a economia compara a corrente média da bateria no teto aprendido com a do
  teto mais alto (RK817).

Arquivos: `module.toml`, `main.sh` (o laço JSON-RPC), `tune.jq` (a lógica),
`locales/` (en, pt_BR, es, ru), `icons/` (Lucide 0.468.0) e `preview.png`.
O aprendizado fica em `games.json`, na pasta de dados do módulo.

Guia completo: `docs/modulos.md`, seção 9. Teste no PC:
`bash examples/modules/ajuste-cpu.test.sh`.

Publicar: tag `v1.0.0`, `tar -czf ajuste-cpu-1.0.0.tar.gz ajuste-cpu/`, e os
assets da release (`ajuste-cpu-1.0.0.tar.gz`, `module.toml`, `preview.png`),
no repositório `archr-linux/archr-ajuste-cpu` com o tópico
`archr-launcher-module`.
