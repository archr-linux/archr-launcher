//! Appearance's box art choices over the shipped themes: the art keeps the
//! edge away from the list, and the list beside it keeps its gap.
use archr_ui::{appearance::Appearance, layout::Layout, Theme};
use std::path::Path;

fn games(theme: &str, look: &Appearance) -> (Layout, Layout) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
    let default = Theme::load(&root.join("default"));
    let theme = Theme::load_over(&root.join(theme), &default);
    let plain = theme.view(&theme.active_view).games;
    let styled = look.apply(&theme);
    (plain, styled.view(&styled.active_view).games)
}

fn look(width: &str, art: &str) -> Appearance {
    Appearance {
        art_width: width.into(),
        game_art: art.into(),
        ..Appearance::default()
    }
}

#[test]
fn art_left_of_the_list_keeps_its_left_edge_and_pushes_the_list() {
    // praia: the art on the left, the list on the right.
    let (plain, wide) = games("praia", &look("50", "1"));
    let (old, art) = (plain.art.unwrap().rect, wide.art.unwrap().rect);
    assert!(
        old.x + old.w <= plain.list.rect.x,
        "praia has its art on the left"
    );
    assert!((art.x - old.x).abs() < 1e-4);
    assert!((art.w - 0.5).abs() < 1e-4);
    let gap = |l: &Layout, a: f32| l.list.rect.x - a;
    assert!((gap(&wide, art.x + art.w) - gap(&plain, old.x + old.w)).abs() < 1e-4);
    // The list keeps its right edge.
    let end = |l: &Layout| l.list.rect.x + l.list.rect.w;
    assert!((end(&wide) - end(&plain)).abs() < 1e-4);
}

#[test]
fn art_off_gives_the_list_the_width_on_either_side() {
    for theme in ["default", "praia"] {
        let (plain, bare) = games(theme, &look("", "0"));
        assert!(bare.art.is_none(), "{theme}");
        let list = bare.list.rect;
        assert!(
            (list.x * 2. + list.w - 1.).abs() < 1e-4,
            "{theme}: {list:?}"
        );
        assert!(list.w > plain.list.rect.w, "{theme}");
    }
}
