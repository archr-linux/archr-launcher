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
