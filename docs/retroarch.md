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
  são gravados uma vez, no primeiro jogo, a partir do controle embutido. Depois
  só mudam por você: dentro do RetroArch (e passam a durar) ou por
  Configurações › Controles › Atalhos › "Restaurar atalhos do RetroArch", que
  volta ao padrão do aparelho. Os atalhos do launcher (sair, volume, brilho,
  captura, MangoHud) são outra coisa e não mexem nos do RetroArch.

## Importar do RetroArch

Configurações › Sistema › Importar do RetroArch traz a configuração de outro
RetroArch (outro aparelho, o PC).

1. Copie para `roms/retroarch-import/` (pela rede, pelo cartão ou por USB):
   - `retroarch.cfg`;
   - `config/<núcleo>/` com as substituições (`.cfg`) e opções de núcleo (`.opt`);
   - `remaps/<núcleo>/*.rmp`;
   - `shaders/` com os presets (`.slangp`, `.glslp`), os shaders que eles
     usam (`.slang`, `.glsl`, `.h`, `.inc`) e as texturas (`.png`, `.jpg`…);
   - `overlays/` com os overlays (`.cfg` e imagens).

   Uma pasta do RetroArch copiada inteira (por exemplo
   `roms/retroarch-import/RetroArch/`) também serve, quando é a única pasta
   lá dentro.

   Overlays e shaders que você já guarda em `roms/overlays/` e `roms/shaders/`
   também são lidos, com as mesmas regras; essas pastas ficam como estão. Se o
   mesmo arquivo estiver nos dois lugares, vale o de `roms/retroarch-import/`.

   Também valem as pastas `shaders/` e `overlays/` dentro da pasta de cada
   sistema (`roms/snes/shaders/`, `roms/n64/overlays/`…), só nesse nível: as
   subpastas de ROMs nunca são vasculhadas. Essas entram numa subpasta com o
   nome do sistema (`roms/snes/shaders/crt/x.slangp` vira
   `/storage/shaders/snes/crt/x.slangp`, que o RetroArch mostra em
   `/tmp/shaders/snes/`), então dois sistemas nunca se sobrescrevem. A
   estrutura interna é copiada igual, então caminhos relativos de presets e
   de overlays `.cfg` continuam valendo.
2. Abra a página: ela mostra o que foi encontrado. "Ver o que será aplicado"
   lista cada ajuste e o que ficou de fora, com o motivo.
3. "Fazer backup antes" (ligado por padrão) salva o estado atual; restaure em
   Armazenamento › Restaurar backup.
4. "Importar agora" vale a partir do próximo jogo. Antes de escrever, o
   launcher confere que a pasta é a mesma da revisão e guarda o conteúdo de
   cada arquivo que vai trocar; se uma escrita falhar, as anteriores são
   desfeitas. Uma queda de energia no meio pode deixar parte aplicada: para
   isso serve o backup.

O que entra:

- do `retroarch.cfg`, só uma lista de chaves seguras (escala inteira,
  proporção, suavização, shader ligado/desligado, combinação de
  botões do menu, posição e opacidade do overlay, latência de áudio, FPS,
  rewind, run-ahead, savestates, aparência do menu), gravadas no
  `retroarch.cfg` do aparelho, nunca no `system.cfg`. Atalhos por número de
  botão não entram: são do controle do outro aparelho. Viewport personalizado,
  rotação, atraso de quadro, tamanho de fonte e "salvar ao sair" também não:
  dependem da tela e do processador de origem, ou são escolha sua aqui;
- substituições e opções só de núcleos instalados;
- presets de shader cujos arquivos existem no aparelho ou vêm na importação;
  os caminhos são reescritos para o diretório de shaders do ArchR;
- shaders, texturas e overlays copiados como estão, nas mesmas subpastas, para
  `/storage/shaders` e `/storage/overlays` (o que o RetroArch vê em
  `/tmp/shaders` e `/tmp/overlays`). Um arquivo que o sistema já traz com o
  mesmo nome fica como está, e um que já está igual no aparelho é pulado.
  Limites: 16 MB por arquivo, 2000 arquivos e 256 MB por importação.

O que nunca entra: caminhos e diretórios, drivers, modo de vídeo do aparelho
(tela cheia, resolução, taxa de atualização, `video_threaded`), qual controle é
qual jogador, contas, senhas e tokens, comandos (`*_cmd`), `#include`, arquivos
grandes demais, que não sejam texto UTF-8 ou que sejam atalhos (links).
