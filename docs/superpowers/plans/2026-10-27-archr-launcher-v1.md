# archr-launcher v1 — Plano de implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Launcher de jogos em Rust + SDL2 que lista sistemas e jogos a partir de um índice em cache, mostra capas quando existem e entrega o jogo ao `runemu.sh` por um supervisor de sessão.

**Architecture:** Workspace Cargo com três bibliotecas (`archr-config`, `archr-index`, `archr-ui`) e um binário (`archr-launcher`). A lógica de telas é uma máquina de estados pura, testável sem SDL; o SDL2 fica isolado em `archr-ui::render`. Um script de shell (`session/archr-launcher-session`) roda o launcher, executa o comando que ele grava em `/run/archr-launcher/next` e o reabre.

**Tech Stack:** Rust (edição 2021), crates `sdl2` (features `image`, `ttf`), `quick-xml`, `toml`, `serde`, `bincode`, `fs2`, `tempfile` (testes). Shell POSIX e systemd para a sessão.

**Spec:** `docs/superpowers/specs/2026-10-27-archr-launcher-design.md`

## Global Constraints

- Primeira tela em menos de 0,3 s depois de o processo começar, com índice em cache (R36S, sonda `swaymsg -t get_tree` a cada 100 ms).
- Comando de jogo sempre no contrato atual: `/usr/bin/runemu.sh <ROM> -P<sistema> --core=<núcleo> --emulator=<emulador> --controllers="<config>"`.
- `system.cfg` em `/storage/.config/system/configs/system.cfg`; trava `/tmp/.system.cfg.lock`; gravação por temporário + `rename`.
- Prioridade de leitura: `<sistema>["<rom>"].<chave>` > `<sistema>.<chave>` > `global.<chave>`.
- Sistemas e emuladores vêm de `/usr/config/emulationstation/es_systems.cfg`; o launcher nunca o escreve.
- Índice e miniaturas em `/storage/.cache/archr-launcher/`; favoritos e recentes em `/storage/.config/archr-launcher/`.
- Mídias: caminhos do `gamelist.xml`; senão `<sistema>/media/<tipo>/<nome-da-rom>.*`; senão `<sistema>/images/<nome-da-rom>.*`.
- Temas: padrão em `/usr/share/archr-launcher/themes/`, usuário em `/storage/.config/archr-launcher/themes/`; `theme.toml`; tema padrão também embutido no binário.
- Botões: A abre, B volta, X favorita, Y menu rápido, L/R pulam página.
- Sem código de NextUI/MinUI. Commits sem atribuição de IA.

## Review Focus

1. **ROM com espaços, aspas, `$` e parênteses no nome** → o comando gravado em `next` chega intacto ao `runemu.sh` (teste na Task 7).
2. **Pasta de sistema inexistente ou sem permissão** → sistema escondido, sem travar (teste na Task 3).
3. **`gamelist.xml` malformado ou com caminhos `./media/...`** → fallback por nome; caminhos relativos à pasta do sistema (teste na Task 3).
4. **`system.cfg` alterado por um script durante a gravação** → nenhuma linha perdida; trava respeitada (teste na Task 1).
5. **Tema com campo inválido** → só aquele campo cai no padrão (teste na Task 5).

---

### Task 1: Workspace e `archr-config`

**Files:** Create `Cargo.toml` (workspace), `.gitignore` (`/target`), `crates/archr-config/{Cargo.toml,src/lib.rs,tests/system_cfg.rs}`

**Interfaces (Produces):**
- `pub struct SystemCfg` (linhas preservadas na ordem)
- `SystemCfg::load(path: &Path) -> io::Result<SystemCfg>`
- `SystemCfg::get(&self, system: &str, rom: Option<&str>, key: &str) -> Option<String>`
- `SystemCfg::set_and_save(path: &Path, lock: &Path, entry_key: &str, value: &str) -> io::Result<()>` (relê sob trava, troca só a linha, temporário + `rename`)
- `pub fn rom_key(system: &str, rom_file_name: &str, key: &str) -> String` → `snes["Chrono Trigger (USA).sfc"].core`

- [ ] **Step 1: Testes que falham:** `get_prefers_game_over_system_over_global` (`global.core=a`, `snes.core=b`, `snes["x.sfc"].core=c` → com rom `c`, sem rom `b`, `nes` → `a`); `set_preserves_other_lines_and_order`; `set_rereads_under_lock` (linha escrita por outro processo após o `load` sobrevive); `rom_key_quotes_name`.
- [ ] **Step 2:** `cargo test -p archr-config` → FAIL.
- [ ] **Step 3:** Implementar; trava com `fs2::FileExt::lock_exclusive`; temporário no mesmo diretório.
- [ ] **Step 4:** → PASS.
- [ ] **Step 5:** Commit `config: read and write system.cfg like the shell helpers, under their lock`.

### Task 2: `archr-index` — `es_systems.cfg`

**Files:** Create `crates/archr-index/{Cargo.toml,src/lib.rs,src/systems.rs,tests/systems.rs,tests/data/es_systems.cfg}` (3 sistemas no formato real, um com dois emuladores)

**Interfaces (Produces):**
- `pub struct SystemDef { pub name: String, pub fullname: String, pub path: PathBuf, pub extensions: Vec<String>, pub emulators: Vec<Emulator> }`
- `pub struct Emulator { pub name: String, pub cores: Vec<Core> }`, `pub struct Core { pub name: String, pub default: bool }`
- `pub fn load_systems(es_systems: &Path) -> Result<Vec<SystemDef>, IndexError>`
- `SystemDef::default_emulator_core(&self) -> Option<(&str, &str)>`

- [ ] **Step 1: Testes que falham:** `parses_three_systems`, `extensions_lowercased_and_split`, `default_core_is_flagged_one`, `missing_file_is_error_not_panic`.
- [ ] **Step 2:** → FAIL. **Step 3:** `quick-xml` em modo de eventos, sem DOM. **Step 4:** → PASS.
- [ ] **Step 5:** Commit `index: read the system list from es_systems.cfg`.

### Task 3: `archr-index` — jogos, nomes limpos e mídias

**Files:** Create `crates/archr-index/src/{games.rs,gamelist.rs}`, `tests/games.rs`, `tests/data/roms/snes/` (ROMs vazias, `gamelist.xml`, `media/images/`, `images/`)

**Interfaces (Produces):**
- `pub struct Game { pub path: PathBuf, pub name: String, pub image: Option<PathBuf>, pub video: Option<PathBuf>, pub favorite: bool, pub last_played: Option<u64> }`
- `pub fn clean_name(file_stem: &str) -> String`
- `pub fn scan_system(sys: &SystemDef) -> Vec<Game>` (ordem alfabética sem diferenciar maiúsculas)

- [ ] **Step 1: Testes que falham:** `clean_name("Super Mario World (USA) [!]") == "Super Mario World"`; `scan_filters_by_extension_case_insensitive`; `gamelist_paths_resolved_relative_to_system`; `fallback_media_then_images_folder`; `malformed_gamelist_falls_back_to_names`; `missing_system_dir_yields_empty_not_error`.
- [ ] **Step 2:** → FAIL. **Step 3:** Ler do `gamelist.xml` só `path`, `name`, `image`, `thumbnail`, `video`, `favorite`, `lastplayed`. **Step 4:** → PASS.
- [ ] **Step 5:** Commit `index: scan games with clean names and ES-compatible media lookup`.

### Task 4: `archr-index` — cache binário

**Files:** Create `crates/archr-index/src/cache.rs`, `tests/cache.rs`

**Interfaces (Produces):**
- `pub const CACHE_VERSION: u32 = 1;`
- `pub struct Overview { pub systems: Vec<SystemEntry> }`, `pub struct SystemEntry { pub name: String, pub fullname: String, pub game_count: u32, pub dir_mtime: u64 }`
- `pub fn load_overview(cache_dir: &Path) -> Option<Overview>` (None se ausente, versão diferente ou corrompido)
- `pub fn build_overview(systems: &[SystemDef]) -> Overview`
- `pub fn save_overview(cache_dir: &Path, o: &Overview) -> io::Result<()>` (temporário + `rename`)
- `pub fn load_games(cache_dir: &Path, sys: &SystemDef) -> Vec<Game>` (cache se o `dir_mtime` bater; senão varre e regrava)

- [ ] **Step 1: Testes que falham:** `roundtrip_overview`, `wrong_version_is_ignored`, `truncated_file_is_ignored`, `stale_mtime_triggers_rescan`, `empty_systems_have_zero_count`.
- [ ] **Step 2:** → FAIL. **Step 3:** `bincode` com cabeçalho mágico `ALAX` + versão. **Step 4:** → PASS.
- [ ] **Step 5:** Commit `index: versioned binary cache invalidated by folder mtime`.

### Task 5: `archr-ui` — temas

**Files:** Create `crates/archr-ui/{Cargo.toml,src/lib.rs,src/theme.rs,tests/theme.rs}`, `themes/default/theme.toml`

**Interfaces (Produces):**
- `pub struct Theme { pub background, text, highlight_bg, highlight_text, dim_text: Rgba, pub font: Option<PathBuf>, pub font_size: f32, pub margin: f32, pub list_width: f32, pub cover: Rect01, pub system_screen: SystemScreen }` — tamanhos em fração da altura da tela; `Rect01` em frações (0..1) da tela
- `pub enum SystemScreen { List, Carousel }` (Carousel aceito, tratado como List na v1)
- `Theme::builtin() -> Theme` (`include_str!` de `themes/default/theme.toml`)
- `Theme::load(dir: &Path) -> Theme` (campo inválido ou ausente herda do builtin; registra no log)
- `pub fn resolve_theme(user_dir: &Path, system_dir: &Path, name: &str) -> Theme`

- [ ] **Step 1: Testes que falham:** `builtin_parses`, `invalid_color_field_falls_back_only_that_field`, `missing_dir_returns_builtin`, `user_dir_wins_over_system_dir`.
- [ ] **Step 2:** → FAIL. **Step 3:** `serde` + `toml`; cores `"#rrggbb"`/`"#rrggbbaa"`. **Step 4:** → PASS.
- [ ] **Step 5:** Commit `ui: declarative TOML themes with per-field fallback`.

### Task 6: Máquina de estados das telas

**Files:** Create `crates/archr-launcher/{Cargo.toml,src/state.rs,src/main.rs (stub),tests/state.rs}`

**Interfaces:**
- Consumes: `SystemEntry`, `Game`.
- Produces: `pub enum Button { A, B, X, Y, L, R, Up, Down, Start, Select }`; `pub enum Screen { Systems, Games { system: String }, QuickMenu { system: String, game: usize } }`; `pub enum Outcome { None, Launch { system: String, game: usize }, ToggleFavorite { system: String, game: usize }, Action(SessionAction) }`; `pub enum SessionAction { Shutdown, Reboot, Suspend, OpenSettings }`; `pub trait DataSource { fn systems(&self) -> &[SystemEntry]; fn games(&self, system: &str) -> &[Game]; }`; `pub struct App { pub screen: Screen, pub cursor: usize, pub page_size: usize }`; `App::handle(&mut self, b: Button, data: &dyn DataSource) -> Outcome`

- [ ] **Step 1: Testes que falham:** `a_on_system_enters_games`, `b_on_games_returns_to_same_system_row`, `a_on_game_launches`, `x_toggles_favorite`, `l_r_move_by_page_and_clamp`, `y_opens_quick_menu_and_b_closes`, `cursor_clamped_when_list_empty`, `collections_listed_before_systems` (Favoritos e Recentes).
- [ ] **Step 2:** → FAIL. **Step 3:** Sem dependência de SDL. **Step 4:** → PASS.
- [ ] **Step 5:** Commit `launcher: screen state machine, testable without SDL`.

### Task 7: Favoritos, recentes e entrega à sessão

**Files:** Create `crates/archr-launcher/src/{store.rs,handoff.rs}`, `tests/{store.rs,handoff.rs}`

**Interfaces (Produces):**
- `pub struct Store` (chave `sistema/caminho`): `Store::load(dir: &Path) -> Store`, `Store::save(&self, dir: &Path) -> io::Result<()>`, `toggle_favorite(&mut self, key: &str)`, `push_recent(&mut self, key: &str)` (máximo 30, sem duplicar)
- `pub fn launch_command(rom: &Path, system: &str, core: &str, emulator: &str, controllers: &str) -> String` (aspas simples POSIX em cada argumento)
- `pub enum NextCmd { Game(String), Action(SessionAction) }`; `pub fn write_next(run_dir: &Path, cmd: &NextCmd) -> io::Result<()>` (linha `game <comando>` ou `action <shutdown|reboot|suspend|settings>`)
- `pub fn read_last_exit(run_dir: &Path) -> Option<i32>`

- [ ] **Step 1: Testes que falham:** `launch_command_survives_quotes_spaces_dollar_parens` (ROM `Tom's $Game (USA).zip`; executar o comando em `sh -c` com um `runemu.sh` falso que imprime `$1` e comparar), `next_file_format_game_and_action`, `recent_dedups_and_caps_at_30`, `favorites_roundtrip`.
- [ ] **Step 2:** → FAIL. **Step 3:** Implementar. **Step 4:** → PASS.
- [ ] **Step 5:** Commit `launcher: favorites, recents and the hand-off file for the session`.

### Task 8: Renderização SDL2, entrada e capas

**Files:** Create `crates/archr-ui/src/{render.rs,input.rs,covers.rs}`, `crates/archr-ui/tests/covers.rs`; substituir `crates/archr-launcher/src/main.rs`

**Interfaces:**
- Consumes: `Theme`, `App`, `Outcome`, `Store`, `write_next`.
- Produces: `Ui::new(width: u32, height: u32, windowed: bool) -> Result<Ui, String>`; `Ui::draw(&mut self, app: &App, data: &dyn DataSource, theme: &Theme, cover: Option<&Texture>)`; `pub fn map_event(e: &sdl2::event::Event) -> Option<Button>` (GameController e, no PC, teclado: Enter, Backspace, X, Y, PgUp/PgDn, setas); `CoverCache { request(&[PathBuf]), get(&PathBuf) -> Option<Texture> }` com thread de carga e miniaturas em `thumbs/` nomeadas por hash do caminho + mtime
- `main`: `--windowed` (640x480 no PC); `ARCHR_LAUNCHER_ROOT` como prefixo de `/storage` e `/usr` para testes.

- [ ] **Step 1: Teste que falha:** `thumbnail_cached_by_path_and_mtime`.
- [ ] **Step 2:** → FAIL. **Step 3:** Primeira tela desenhada antes de qualquer carga de capa ou de índice por sistema; música não entra na v1. **Step 4:** → PASS.
- [ ] **Step 5: Verificação no PC:** `ARCHR_LAUNCHER_ROOT=crates/archr-index/tests/data cargo run -p archr-launcher -- --windowed` mostra os 3 sistemas, entra, volta, abre o menu rápido e, com A num jogo, encerra deixando `next` com `game ...`.
- [ ] **Step 6:** Commit `ui: SDL2 rendering, controller input and lazy cover thumbnails`.

### Task 9: Sessão, empacotamento e medição no aparelho

**Files:** Create `session/archr-launcher-session`, `session/archr-launcher-session.service`, `session/test-session.sh`; no repo `distribution`: `projects/ArchR/packages/ui/archr-launcher/package.mk`

**Interfaces:**
- Consumes: formato de `next` e `last-exit` (Task 7).
- Supervisor: espera o sway (socket Wayland + `swaymsg -t get_outputs` com `"active": true`, 100 ms, teto 10 s); laço: roda `/usr/bin/archr-launcher`; lê e apaga `next`; `game` → executa e grava o código em `last-exit`; `action` → `systemctl poweroff|reboot|suspend` ou `/usr/bin/archr-settings`.
- Unidade: `Requires=sway.service`, `After=sway.service`, `Restart=always`, `RestartSec=1`, `EnvironmentFile=/etc/profile`; não habilitada (o ES continua padrão via `UI_SERVICE`).
- `package.mk`: `PKG_DEPENDS_TARGET="toolchain SDL2 SDL2_image SDL2_ttf cargo:host rust:host"`; instala binário, tema padrão, sessão e unidade.

- [ ] **Step 1:** `session/test-session.sh` com `archr-launcher`, `runemu.sh` e `systemctl` falsos: uma volta `game` grava `last-exit`; `action reboot` chama `systemctl reboot`. Rodar → FAIL.
- [ ] **Step 2:** Implementar o supervisor. Rodar → PASS.
- [ ] **Step 3:** Build da distro com o pacote; no aparelho, `UI_SERVICE` apontado à mão para a sessão.
- [ ] **Step 4: Verificação no aparelho:** sonda mede a primeira tela (< 0,3 s após o início do processo, índice em cache); abrir e voltar de um jogo RetroArch, do N64 standalone e de um port; trocar o núcleo pelo menu rápido e conferir o `system.cfg`.
- [ ] **Step 5:** Commits: `session: supervisor that runs the launcher and the chosen game` (archr-launcher) e `archr-launcher: package the launcher next to EmulationStation` (distribution).
