# Avisos (toasts) do archr-launcher

Avisos são cartões pequenos no canto de cima da tela, logo abaixo da pílula de
status, que dizem que algo aconteceu. Aparecem só dentro do launcher (nunca
por cima de um jogo) e não pegam botão nenhum: a tela de baixo continua
funcionando. Ficam no máximo 3 na tela, o mais novo em cima; o 4º espera a
vez. O mesmo aviso de novo (mesmo tipo e mesmo título) não empilha: renova o
tempo do cartão que já está na tela, com o detalhe novo.

## Para scripts do sistema: o spool

Qualquer processo pode pedir um aviso deixando um arquivo `<qualquer>.toast`
em `/run/archr/notify/` (o launcher cria a pasta ao iniciar; quem escreve pode
criá-la também). O launcher olha a pasta uma vez por segundo, mostra o aviso
conforme as configurações e **apaga o arquivo**. Arquivo malformado é apagado
sem aparecer.

O arquivo tem 3 linhas e uma 4ª opcional, em UTF-8, até 4 KiB:

```
done|info|error            tipo: verde, azul (destaque) ou laranja
updates|network|storage|errors   categoria (as chaves "Mostrar" abaixo)
Título                     uma linha, cortada com … se não couber
Detalhe opcional           até duas linhas na tela, cortado com …
```

Escreva com outro nome e renomeie, para o launcher nunca ler um arquivo pela
metade (só `*.toast` é lido):

```sh
mkdir -p /run/archr/notify
f=/run/archr/notify/backup-$$.toast
printf 'done\nstorage\nCópia de segurança concluída\n12 jogos copiados\n' > "$f.tmp"
mv "$f.tmp" "$f"
```

Se o launcher não está rodando (um jogo aberto), o arquivo espera e o aviso
aparece quando ele voltar. O texto vai como está: traduza antes de escrever
(`system.language` no `system.cfg`).

## Configurações › Aparência › Avisos

| opção | chave (`system.cfg`) | valores |
|---|---|---|
| Avisos na tela | `launcher.toasts` | `1` (padrão) / `0` |
| Posição | `launcher.toasts.position` | `right` (padrão) / `left` |
| Duração | `launcher.toasts.duration` | `3`, `4` (padrão), `6`, `8` segundos; erros ficam o dobro |
| Mostrar › Atualizações | `launcher.toasts.updates` | `1` (padrão) / `0` |
| Mostrar › Rede e Bluetooth | `launcher.toasts.network` | `1` (padrão) / `0` |
| Mostrar › Cartão e USB | `launcher.toasts.storage` | `0` (padrão) / `1` |
| Mostrar › Erros | `launcher.toasts.errors` | `1` (padrão) / `0` |

A categoria decide se o aviso aparece; o tipo decide só a cor, o ícone e o
tempo (um `error` da categoria `updates` segue a chave de Atualizações).

## O que o próprio launcher avisa

| aviso | tipo / categoria | quando |
|---|---|---|
| N atualizações disponíveis | info / updates | a verificação (a do início ou outra) achou pacotes; uma vez por boot e versão |
| Atualização de tema: nome / N atualizações de temas | info / updates | a loja de temas tem versão nova de um tema instalado; uma vez por boot e conjunto de versões, nunca na página da loja (vista lá, conta como avisada) |
| Atualização de módulo: nome / N atualizações de módulos | info / updates | idem, para a loja de módulos |
| Atualização concluída / A atualização falhou | done, error / updates | a instalação terminou longe da página Atualizações |
| Wi-Fi conectado (rede) / Wi-Fi desconectado | done, info / network | a rede entrou ou caiu com o rádio ligado; não ao ligar o launcher, não ao desligar o Wi-Fi, não na página Wi-Fi |
| Controle conectado (nome) | done / network | um controle Bluetooth acabou de conectar |
| A coleta de artes parou (motivo) | error / errors | o scraping parou com erro longe da página dele |
| Não deu certo (motivo) | error / errors | uma resposta de Configurações falhou depois que a tela foi fechada (login do ScreenScraper, RetroAchievements…) |

Avisos que já tinham lugar continuam lá, sem cartão repetido: o erro de saída
de um jogo e as mensagens de módulos seguem no aviso central, o adaptador USB
no cartão dele.
