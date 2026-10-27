use archr_ui::{motion::Tween, status::Status};
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn selection_eases_out_and_new_input_can_snap_without_waiting() {
    let now = Instant::now();
    let mut tween = Tween::new(10.0, Duration::from_millis(90));
    tween.retarget(50.0, now);
    assert_eq!(tween.value(now), 10.0);
    assert!(tween.value(now + Duration::from_millis(45)) > 30.0);
    assert!(tween.active(now + Duration::from_millis(89)));
    assert_eq!(tween.value(now + Duration::from_millis(90)), 50.0);
    tween.snap();
    assert!(!tween.active(now));
    assert_eq!(tween.value(now), 50.0);
    tween.retarget(90.0, now);
    assert_eq!(tween.value(now), 50.0);
}

#[test]
fn battery_is_prefixed_optional_and_not_reread_inside_thirty_seconds() {
    let root = tempfile::tempdir().unwrap();
    let battery = root.path().join("sys/class/power_supply/battery");
    fs::create_dir_all(&battery).unwrap();
    fs::write(battery.join("capacity"), "87\n").unwrap();
    fs::write(battery.join("status"), "Charging\n").unwrap();
    let now = Instant::now();
    let mut status = Status::new(Some(root.path()), now);
    let value = status.battery.unwrap();
    assert_eq!(value.percent, 87);
    assert!(value.charging);
    fs::write(battery.join("capacity"), "12").unwrap();
    assert!(!status.refresh_battery(now + Duration::from_secs(29)));
    assert_eq!(status.battery.unwrap().percent, 87);
    assert!(status.refresh_battery(now + Duration::from_secs(30)));
    assert_eq!(status.battery.unwrap().percent, 12);
    fs::write(battery.join("capacity"), "999").unwrap();
    assert!(status.refresh_battery(now + Duration::from_secs(60)));
    assert_eq!(status.battery, None);
    fs::remove_file(battery.join("capacity")).unwrap();
    assert!(!status.refresh_battery(now + Duration::from_secs(90)));
    assert_eq!(status.battery, None);
}
