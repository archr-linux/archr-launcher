# Perfil de CPU

Módulo do [archr-launcher](https://github.com/archr-linux/archr-launcher) para
o ArchR: no menu Y de um jogo, "Perfil de CPU sugerido" diz qual governador de
CPU combina com o sistema do jogo; em Configurações › Sistema › Módulos, uma
página escolhe a sugestão para sistemas pesados e leves e mostra o governador e
a frequência atuais.

- Tipo: `oneshot`, sh + `jq`, API 2.
- Permissões: nenhuma. Só lê `/sys/devices/system/cpu`; não muda nada.
- As escolhas são os quatro governadores do RK3326 que o `runemu.sh` aceita:
  `performance`, `schedutil`, `ondemand` e `powersave`.
- Aplicar o perfil sozinho em cada jogo exige um daemon com
  `hooks = ["launch"]` e `system = ["cpu-profile"]` (veja `docs/modulos.md`).

*Suggests a CPU governor for each game's system from the Y menu and shows the
current one. One-shot, sh + jq, API 2, no permissions.*

## Publicar uma versão

Suba `version` no `module.toml`, faça o commit e crie a tag `v<versão>`; o
pacote sai da própria tag:

```
git archive --format=tar.gz --prefix=perfil-cpu/ -o ../perfil-cpu-<versão>.tar.gz v<versão>
```

Release `v<versão>` com três assets: o `.tar.gz`, `module.toml` e
`preview.png` (640×480).
