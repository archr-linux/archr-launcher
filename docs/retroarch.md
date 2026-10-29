# Configurações do RetroArch

## Quem decide cada opção

- **O launcher só impõe o que você escolheu nele.** Em Configurações › Jogos,
  Proporção, Filtro de imagem e Slot de estado começam em "Padrão do RetroArch":
  nada é imposto e vale o que você salvou no próprio RetroArch. Escolher outro
  valor grava `<sistema>.<opção>` no `system.cfg`; voltar a "Padrão do
  RetroArch" apaga a chave.
- **O que você salva no RetroArch dura.** Escala inteira (inclusive a
  inteligente), shaders, overlay, proporção e o viewport personalizado (como o
  deslocamento Y do GBA), latência e FPS ficam como você deixou, ao sair com
  "Salvar configuração ao sair" ou pelo menu.
- **Substituições de núcleo e de jogo vencem tudo.** No menu rápido do
  RetroArch, "Substituições › Salvar substituição de núcleo/jogo" grava em
  `~/.config/retroarch/config/<núcleo>/` e vale por cima do `retroarch.cfg` e
  do que o launcher definiu. Presets de shader salvos para o núcleo ou o jogo
  também vencem: o launcher só passa um shader quando você escolheu um nele.
- **Atalhos do RetroArch** (menu, salvar/carregar estado, avanço rápido...)
  são gravados uma vez, no primeiro jogo, a partir do controle do aparelho, e de
  novo só quando você muda os atalhos em Configurações › Controles › Atalhos.
  Atalhos trocados dentro do RetroArch passam a durar.

## Importar do RetroArch

Configurações › Sistema › Importar do RetroArch traz a configuração de outro
RetroArch (outro aparelho, o PC).

1. Copie para `roms/retroarch-import/` (pela rede, pelo cartão ou por USB):
   - `retroarch.cfg`;
   - `config/<núcleo>/` com as substituições (`.cfg`) e opções de núcleo (`.opt`);
   - `remaps/<núcleo>/*.rmp`;
   - `shaders/` com os presets (`.slangp`, `.glslp`).
2. Abra a página: ela mostra o que foi encontrado. "Ver o que será aplicado"
   lista cada ajuste e o que ficou de fora, com o motivo.
3. "Fazer backup antes" (ligado por padrão) salva o estado atual; restaure em
   Armazenamento › Restaurar backup.
4. "Importar agora" aplica tudo de uma vez; vale a partir do próximo jogo. Se
   algo falhar no meio, os arquivos voltam ao que eram.

O que entra:

- do `retroarch.cfg`, só uma lista de chaves seguras (escala inteira,
  proporção e viewport, suavização, shader ligado/desligado, atalhos, posição e
  opacidade do overlay, latência de áudio, FPS, rewind, run-ahead, savestates,
  aparência do menu), gravadas no `retroarch.cfg` do aparelho, nunca no
  `system.cfg`;
- substituições e opções só de núcleos instalados;
- presets de shader cujos arquivos existem no aparelho; os caminhos são
  reescritos para o diretório de shaders do ArchR.

O que nunca entra: caminhos e diretórios, drivers, modo de vídeo do aparelho
(tela cheia, resolução, taxa de atualização, `video_threaded`), qual controle é
qual jogador, contas, senhas e tokens, comandos (`*_cmd`), `#include`, arquivos
grandes demais, que não sejam texto UTF-8 ou que sejam atalhos (links).
