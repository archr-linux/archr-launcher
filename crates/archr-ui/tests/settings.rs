use archr_ui::{
    settings::{Effect, Id, Page, Settings},
    state::Button,
};

#[test]
fn breadcrumbs_restore_focus_and_root_back_closes() {
    let mut s = Settings::default();
    s.loading = false;
    s.cursor = 1;
    assert_eq!(s.handle(Button::A), Effect::None);
    assert_eq!(s.page(), Page::Sound);
    s.handle(Button::Down);
    s.handle(Button::B);
    assert_eq!(s.cursor, 1);
    assert_eq!(s.handle(Button::B), Effect::Close);
}

#[test]
fn sliders_clamp_and_busy_rows_cannot_be_changed_twice() {
    let mut s = Settings::default();
    s.loading = false;
    s.open(Page::Sound);
    s.values.insert(Id::Volume, "100".into());
    assert_eq!(s.handle(Button::R), Effect::None);
    assert_eq!(s.handle(Button::L), Effect::Set(Id::Volume, "95".into()));
    assert_eq!(s.handle(Button::L), Effect::None);
    assert_eq!(s.value(Id::Volume), "100"); // committed only after worker success
}

#[test]
fn system_info_shortcut_and_gpu_reboot_confirmation() {
    let mut s = Settings::default();
    s.loading = false;
    s.handle(Button::Y);
    assert_eq!(s.page(), Page::Info);
    s.handle(Button::B);
    s.open(Page::System);
    assert_eq!(s.handle(Button::R), Effect::Set(Id::Gpu, "panfrost".into()));
    s.complete(Id::Gpu, Ok("panfrost".into()));
    assert!(s.reboot_prompt);
    assert_eq!(s.handle(Button::B), Effect::None);
    assert!(!s.reboot_prompt);
}

#[test]
fn malformed_negative_and_extreme_slider_values_are_normalized_before_stepping() {
    for (stored, normalized, right) in [
        ("oops", "60", "65"),
        ("--1", "60", "65"),
        ("+-1", "60", "65"),
        ("-200", "0", "5"),
        ("32767", "100", "100"),
        ("999999999999999999999999999999999999999999", "100", "100"),
    ] {
        assert_eq!(Id::Volume.normalize(stored), normalized);
        let mut s = Settings::default();
        s.loading = false;
        s.open(Page::Sound);
        s.values.insert(Id::Volume, stored.into());
        assert_eq!(s.handle(Button::R), Effect::Set(Id::Volume, right.into()));
    }
    assert_eq!(Id::Brightness.normalize("-32768"), "5");
}

#[test]
fn localized_values_and_footer_affordances_match_actions() {
    use archr_ui::HintButton::{Key, Pair};
    let ru = archr_ui::i18n::I18n::load(std::path::Path::new("/nonexistent"), Some("ru"), None);
    let mut s = Settings::default();
    s.loading = false;
    s.open(Page::Sound);
    assert_eq!(
        s.hints().iter().map(|h| h.0).collect::<Vec<_>>(),
        [Pair('L', 'R'), Key('B')]
    );
    s.values.insert(Id::Idle, "5".into());
    assert_eq!(s.display_value(Id::Idle, &ru), "5 мин");
    s.open(Page::System);
    for (value, key) in [
        ("off", "settings.off"),
        ("mem", "settings.suspend_mem"),
        ("freeze", "settings.suspend_freeze"),
        ("standby", "settings.suspend_standby"),
        ("default", "menu.default"),
    ] {
        s.values.insert(Id::Suspend, value.into());
        assert_eq!(s.display_value(Id::Suspend, &ru), ru.text(key));
        assert_ne!(s.display_value(Id::Suspend, &ru), value);
    }
    s.cursor = 3;
    assert_eq!(
        s.hints().iter().map(|h| h.0).collect::<Vec<_>>(),
        [Pair('L', 'R'), Key('A'), Key('B')]
    );
    s.cursor = 4;
    assert_eq!(
        s.hints().iter().map(|h| h.0).collect::<Vec<_>>(),
        [Key('A'), Key('B')]
    );
    s.open(Page::Info);
    assert_eq!(
        s.hints(),
        &[(Key('A'), "settings.details"), (Key('B'), "hint.back")]
    );
}

#[test]
fn information_grid_moves_in_two_columns_and_details_restore_focus() {
    let mut state = Settings::default();
    state.open(Page::Info);
    state.handle(Button::R);
    state.handle(Button::Up);
    assert_eq!(state.cursor, 1, "up preserves the column at the top edge");
    state.handle(Button::L);
    state.handle(Button::Down);
    assert_eq!(state.cursor, 2);
    state.handle(Button::Down);
    state.handle(Button::Down);
    assert_eq!(
        state.cursor, 4,
        "down preserves the column at the bottom edge"
    );
    state.handle(Button::Up);
    state.handle(Button::L);
    assert_eq!(
        state.cursor, 2,
        "left edge cannot jump into the previous row"
    );
    state.handle(Button::R);
    state.handle(Button::R);
    assert_eq!(state.cursor, 3, "right edge cannot jump into the next row");
    state.handle(Button::A);
    assert_eq!(state.page(), Page::InfoDetail(3));
    state.handle(Button::R);
    assert_eq!(state.page(), Page::InfoDetail(4));
    state.handle(Button::L);
    assert_eq!(state.page(), Page::InfoDetail(3));
    state.handle(Button::B);
    assert_eq!(state.page(), Page::Info);
    assert_eq!(state.cursor, 3);
    state.handle(Button::B);
    assert_eq!(state.page(), Page::Root);
}
