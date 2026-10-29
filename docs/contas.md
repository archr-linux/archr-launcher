# Contas: RetroAchievements e ScreenScraper

Onde cada login fica, o que sobrevive a um reinício e o que backup,
restauração e restauração de fábrica fazem com ele.

## Onde fica

| Dado | Arquivo | Modo |
|---|---|---|
| Usuário e token do RetroAchievements | `/storage/.config/system/configs/system.cfg` (`global.retroachievements.username` e `.token`) | o do system.cfg |
| Chave da API web do RetroAchievements (opcional) | `/storage/.config/archr-launcher/cheevos-web-key` | 0600 |
| Usuário e senha do ScreenScraper | `/storage/.config/archr-launcher/scraper.toml` | 0600 |

A senha do RetroAchievements nunca é guardada: o login a troca por um token.
A chave da API web fica fora do system.cfg de propósito: o backuptool copia o
system.cfg para o compartilhamento das ROMs, e módulos podem receber chaves do
system.cfg por `config_read`. Nenhum dos dois vê o arquivo da chave.

## Reinício e desligamento

- O estado "conectado" vem só do disco, ao abrir o launcher e ao abrir cada
  página. A rede só atualiza pontos, "membro desde" e o último jogo.
- Toda gravação é atômica e durável: arquivo temporário na mesma pasta,
  `fsync` do arquivo, `rename`, `fsync` da pasta (e das pastas criadas no
  caminho). O system.cfg passa pela trava do archr-config. Um corte de energia
  deixa o arquivo antigo ou o novo, nunca um pela metade.
- Sem rede, tempo esgotado, cota estourada ou serviço fora do ar não apagam
  nada. Um token recusado pelo serviço também não apaga nada: a página mostra
  "Sessão expirada" e pede só a senha, com o usuário já preenchido. Uma chave
  da API nova só é salva depois que o serviço a aceita; uma chave salva que o
  serviço passe a recusar continua salva, marcada como recusada, até ser
  trocada ou removida.
- Só "Sair da conta" apaga o token e a chave da API web do RetroAchievements
  (o nome do usuário fica no system.cfg). O ScreenScraper não tem saída: um
  login novo só substitui o salvo depois que o serviço o aceita.

## Backup e restauração (backuptool)

- O backup inclui o system.cfg, portanto o usuário e o token do
  RetroAchievements vão para `/storage/roms/backup/*.zip`, sem criptografia.
- `scraper.toml` e `cheevos-web-key` não entram no backup.
- A restauração sobrescreve o system.cfg com o do zip: o login do
  RetroAchievements volta a ser o daquele backup (ou some, se o backup foi
  feito desconectado). Se o token antigo não valer mais, a página pede a senha.
  O login do ScreenScraper e a chave da API ficam como estavam.

## Restauração de fábrica

`factoryreset ALL` apaga todo o `/storage` exceto ROMs e a pasta de
atualização: os três arquivos somem e as duas contas precisam ser conectadas
de novo. Os outros modos (`retroarch`, `standalone`, `audio`…) não tocam
nesses arquivos; o `setsettings.sh` reescreve o token no `retroarch.cfg` a
cada jogo a partir do system.cfg.
