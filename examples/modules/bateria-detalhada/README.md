# Bateria detalhada

Módulo do [archr-launcher](https://github.com/archr-linux/archr-launcher) para
o ArchR: um bloco no menu Start com a carga e a potência da bateria, e uma
página em Configurações › Sistema › Módulos com o estado, a tensão e a saúde
da bateria, a opção de mostrar a potência e o intervalo de atualização.

- Tipo: `daemon`, bash + `jq`, API 2.
- Permissões: nenhuma. Só lê `/sys/class/power_supply`.
- Sem bateria (num PC), o bloco some.

Instale pela Loja de módulos do launcher (Configurações › Sistema › Módulos ›
Loja). Guia de módulos: `docs/modulos.md` no repositório do launcher.

*A Start menu tile with battery charge and power, and a settings page with
state, voltage and health. Daemon, bash + jq, API 2, no permissions.*

## Publicar uma versão

Suba `version` no `module.toml`, faça o commit e crie a tag `v<versão>`; o
pacote sai da própria tag:

```
git archive --format=tar.gz --prefix=bateria-detalhada/ -o ../bateria-detalhada-<versão>.tar.gz v<versão>
```

Release `v<versão>` com três assets: o `.tar.gz`, `module.toml` e
`preview.png` (640×480).
