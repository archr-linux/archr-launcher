# Ajuste automático de CPU

Módulo oficial do archr-launcher (API 3, bash + jq, daemon). Aprende, jogo a
jogo, o menor teto de frequência da CPU que mantém o jogo na velocidade total,
e economiza bateria com ele. Só a CPU e só naquela abertura: nunca a GPU, a
memória ou o seu ajuste de governador.

Como aprende:

- antes de cada jogo responde ao `launch.before` com o teto que está testando
  (`cpu_max_khz`, um dos `cpu.opps` do `initialize`);
- na volta, o `game.ended` traz a medida do `archr-perf-probe` (ocupação da
  thread mais ocupada do emulador, segundos no limite, corrente da bateria);
- sessão sem perda (p95 abaixo de 92 % e menos de 2 s no limite): desce um
  degrau; com perda: sobe um e marca o piso; três sessões estáveis no mesmo
  teto: "aprendido". Um jogo aprendido que ficar lento volta um degrau;
- sessões com menos de dois minutos não contam; N64, PSP, Dreamcast e Saturn,
  e jogos lentos já no máximo, ficam fora;
- a economia compara a corrente média da bateria no teto aprendido com a do
  teto mais alto (RK817).

Arquivos: `module.toml`, `main.sh` (o laço JSON-RPC), `tune.jq` (a lógica),
`locales/` (en, pt_BR, es, ru), `icons/` (Lucide 0.468.0) e `preview.png`.
O aprendizado fica em `games.json`, na pasta de dados do módulo.

Guia completo: `docs/modulos.md`, seção 9.

Publicar: tag `v1.0.0`, `tar -czf ajuste-cpu-1.0.0.tar.gz ajuste-cpu/`, e os
assets da release (`ajuste-cpu-1.0.0.tar.gz`, `module.toml`, `preview.png`),
no repositório `archr-linux/archr-ajuste-cpu` com o tópico
`archr-launcher-module`.
