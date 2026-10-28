#[test]
fn controller_arguments_preserve_runemu_spaces_and_device_indices() {
    assert_eq!(
        archr_ui::controllers::controller_arguments([(2, "abc".into()), (5, "def".into())]),
        " -p1index 2 -p1guid abc  -p2index 5 -p2guid def "
    );
}
