//! Shared event-loop terminal transition, including delayed core-write handoffs.
use archr_launcher::{
    library::Library,
    state::{App, DataSource, Outcome, SessionAction},
};
use archr_ui::{render::Ui, Theme};

/// Blocking durability work is allowed only at a terminal transition: present
/// immediate feedback first, then flush Store and publish the durable handoff.
/// A persistence/handoff failure cancels exit and restores the launcher/banner.
/// `None` requests window-close (Store flush only, without supervisor handoff).
pub fn apply(
    ui: &mut Ui<'_, '_>,
    app: &App,
    library: &mut Library,
    theme: &Theme,
    outcome: Option<Outcome>,
    controllers: &str,
    cover: Option<&sdl2::render::Texture<'_>>,
) -> Result<bool, String> {
    ui.transition = message(outcome.as_ref(), library);
    if ui.transition.is_some() {
        ui.draw(app, library, theme, cover)?; // draw includes present, before any flush
    }
    let result = match outcome {
        Some(outcome) => library.handle(outcome, controllers),
        None => library.finish().map(|()| true),
    };
    ui.transition = None;
    match result {
        Ok(exit) => Ok(exit),
        Err(error) => {
            ui.banner = Some(error);
            ui.draw(app, library, theme, cover)?;
            Ok(false)
        }
    }
}
fn message(outcome: Option<&Outcome>, data: &dyn DataSource) -> Option<String> {
    Some(match outcome {
        None => "Fechando…".to_owned(),
        Some(Outcome::Launch { system, game }) => format!(
            "Abrindo {}…",
            data.games(system)
                .get(*game)
                .map(|game| game.name.as_str())
                .unwrap_or("jogo")
        ),
        Some(Outcome::Action(action)) => match action {
            SessionAction::Shutdown => "Desligando…",
            SessionAction::Reboot => "Reiniciando…",
            SessionAction::Suspend => "Suspendendo…",
            SessionAction::OpenSettings => "Abrindo configurações…",
        }
        .to_owned(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use archr_launcher::{paths::Paths, state::SessionAction};
    use std::{
        fs,
        time::{Duration, Instant},
    };

    #[test]
    fn failed_handoff_presents_feedback_then_sets_banner_without_exiting() {
        std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::new(Some(root.path().to_owned()));
        fs::create_dir_all(paths.run.parent().unwrap()).unwrap();
        // An unwritable handoff location, even when tests run as root.
        fs::write(&paths.run, "not a directory").unwrap();
        let mut library = Library::initial(paths.clone());
        library.start();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !library.ready {
            library.poll();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let ttf;
        let creator;
        let mut ui = Ui::new(640, 480, true).unwrap();
        ttf = sdl2::ttf::init().unwrap();
        creator = ui.canvas.texture_creator();
        let theme = Theme::builtin();
        ui.configure(&ttf, &creator, &theme, None).unwrap();
        let frame = root.path().join("transition.png");
        ui.screenshot = Some(frame.clone());
        let exit = apply(
            &mut ui,
            &App::new(10),
            &mut library,
            &theme,
            Some(Outcome::Action(SessionAction::OpenSettings)),
            "",
            None,
        )
        .unwrap();
        assert!(!exit, "failed handoff must return to the event loop");
        assert!(ui.banner.as_ref().is_some_and(|banner| !banner.is_empty()));
        assert!(
            ui.transition.is_none(),
            "failure must dismiss the busy overlay"
        );
        assert!(!paths.run.join("next").exists());
        use sdl2::image::LoadSurface;
        let image = sdl2::surface::Surface::from_file(frame)
            .unwrap()
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        let color = theme.surface_elevated;
        let offset = 220 * image.pitch() as usize + 140 * 4;
        image.with_lock(|bytes| {
            assert_eq!(
                &bytes[offset..offset + 4],
                &[color.r, color.g, color.b, color.a]
            )
        });
        assert!(
            paths.store.join("store.toml").exists(),
            "handoff follows the store flush"
        );
        // Flush failures must also cancel action and window-close transitions.
        fs::remove_file(paths.store.join("store.toml")).unwrap();
        fs::create_dir(paths.store.join("store.toml")).unwrap();
        for outcome in [Some(Outcome::Action(SessionAction::Shutdown)), None] {
            ui.banner = None;
            assert!(!apply(
                &mut ui,
                &App::new(10),
                &mut library,
                &theme,
                outcome,
                "",
                None
            )
            .unwrap());
            assert!(ui.banner.is_some());
            assert!(ui.transition.is_none());
        }
        // Repair the destinations and verify the same launcher can retry.
        fs::remove_dir(paths.store.join("store.toml")).unwrap();
        fs::remove_file(&paths.run).unwrap();
        assert!(apply(
            &mut ui,
            &App::new(10),
            &mut library,
            &theme,
            Some(Outcome::Action(SessionAction::OpenSettings)),
            "",
            None
        )
        .unwrap());
        assert_eq!(
            fs::read_to_string(paths.run.join("next")).unwrap(),
            "action settings\n"
        );
    }
    #[test]
    fn terminal_labels_match_requested_actions_and_display_game_name() {
        use archr_index::{cache::SystemEntry, Game};
        struct Data(Vec<Game>);
        impl DataSource for Data {
            fn systems(&self) -> &[SystemEntry] {
                &[]
            }
            fn games(&self, _: &str) -> &[Game] {
                &self.0
            }
            fn origin(&self, _: &str, _: usize) -> Option<(String, usize)> {
                None
            }
            fn cores(&self, _: &str) -> Vec<(String, String)> {
                vec![]
            }
            fn current_core(&self, _: &str, _: usize) -> Option<(String, String)> {
                None
            }
        }
        let data = Data(vec![Game {
            name: "Super Mario".into(),
            path: "game.nes".into(),
            image: None,
            video: None,
            favorite: false,
            year: None,
            players: None,
            last_played: None,
        }]);
        assert_eq!(
            message(
                Some(&Outcome::Launch {
                    system: "nes".into(),
                    game: 0
                }),
                &data
            )
            .as_deref(),
            Some("Abrindo Super Mario…")
        );
        for (action, expected) in [
            (SessionAction::Shutdown, "Desligando…"),
            (SessionAction::Reboot, "Reiniciando…"),
            (SessionAction::Suspend, "Suspendendo…"),
            (SessionAction::OpenSettings, "Abrindo configurações…"),
        ] {
            assert_eq!(
                message(Some(&Outcome::Action(action)), &data).as_deref(),
                Some(expected)
            );
        }
        assert_eq!(message(None, &data).as_deref(), Some("Fechando…"));
        assert_eq!(message(Some(&Outcome::None), &data), None);
    }
}
