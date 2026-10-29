# Temas

## Sons

Um tema pode ter sons: de movimentação, de ação e de cada sistema. Tudo é
dado do tema; nenhum som é exigido, e um tema sem sons fica em silêncio.

### O jeito mais simples

Crie a pasta `sounds/` no tema e coloque arquivos `.ogg` ou `.wav` com o nome
do evento. Não é preciso declarar nada no `theme.toml`.

| Arquivo        | Toca quando                                   |
|----------------|-----------------------------------------------|
| `boot`         | o launcher abre                               |
| `move`         | o cursor muda de item                         |
| `page`         | pula página (L/R) ou letra                    |
| `select`       | abre um sistema, página ou item (A)           |
| `back`         | volta (B)                                     |
| `launch`       | abre um jogo, ferramenta ou stream            |
| `menu_open`    | abre o menu rápido (Y)                        |
| `menu_close`   | fecha o menu rápido                           |
| `toggle`       | muda um valor nas configurações               |
| `error`        | uma operação falha                            |
| `notice`       | aparece um aviso                              |
| `favorite`     | marca ou desmarca favorito                    |

Evento sem arquivo fica em silêncio.

### Sons por sistema

`sounds/systems/<nome-do-sistema>.ogg` (ou `.wav`) toca ao parar o cursor
naquele sistema, por exemplo `sounds/systems/snes.ogg`. O som só toca depois
de uns 350 ms parado, então rolar a lista rápido não dispara nada. O sistema
mostrado ao abrir o launcher não toca.

### Tabela `[sounds]` (opcional)

Só é necessária para trocar o nome de um arquivo ou silenciar um evento que
tem arquivo. Os caminhos são relativos ao tema e não saem dele.

```toml
[sounds]
move = "sounds/tick.wav"   # outro arquivo
select = ""                # silêncio, mesmo existindo sounds/select.ogg

[sounds.systems]
snes = "sounds/jingle.ogg"
nes = ""
```

### Limites e formato

- WAV ou OGG Vorbis, curtos (até 1 MiB por arquivo); mono ou estéreo.
- Todos os sons são carregados na abertura, fora da thread de desenho.
- O volume (0 a 100%) e o interruptor ficam em Configurações > Som e tela
  ("Sons do tema" e "Volume dos sons"); o volume é relativo ao do sistema.
- O launcher libera a saída de áudio antes de abrir um jogo ou ferramenta,
  e a reabre ao voltar.
- O tema `default` traz um conjunto sintetizado (ver `themes/default/sounds/README.md`)
  e nenhum som por sistema.
