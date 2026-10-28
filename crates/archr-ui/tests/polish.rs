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
    assert_eq!(
        status.battery, None,
        "no sysfs read before the first present"
    );
    status.poll(now + Duration::from_secs(90));
    assert_eq!(status.battery, None);
    status.start();
    status.start(); // idempotent: never spawn a second poller
    let deadline = Instant::now() + Duration::from_secs(2);
    while status.battery.is_none() {
        status.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let value = status.battery.unwrap();
    assert_eq!(value.percent, 87);
    assert!(value.charging);
    fs::write(battery.join("capacity"), "12").unwrap();
    status.poll(now + Duration::from_secs(90));
    assert_eq!(
        status.battery.unwrap().percent,
        87,
        "render ticks cannot trigger sysfs reads"
    );
}

#[test]
fn slow_sysfs_worker_never_blocks_construction_polling_or_drop() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let root = tempfile::tempdir().unwrap();
    let battery = root.path().join("sys/class/power_supply/battery");
    fs::create_dir_all(&battery).unwrap();
    let path = battery.join("capacity");
    let name = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: name is a live NUL-terminated path in our private test directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(archr_ui::status::read_battery(root.path()).is_none());
    let began = Instant::now();
    let mut status = Status::new(Some(root.path()), began);
    status.start();
    for _ in 0..10 {
        status.poll(Instant::now());
    }
    assert!(began.elapsed() < Duration::from_millis(100));
    assert_eq!(status.battery, None);
    let began = Instant::now();
    drop(status); // do not join a worker blocked inside sysfs
    assert!(began.elapsed() < Duration::from_millis(100));
}

#[test]
fn page_sized_sysfs_metadata_keeps_battery_acquisition_working() {
    let root = tempfile::tempdir().unwrap();
    let battery = root.path().join("sys/class/power_supply/rk817-battery");
    fs::create_dir_all(&battery).unwrap();
    // Pad short attributes to the length reported by sysfs, not their text length.
    for (name, value) in [
        ("type", "Battery\n"),
        ("capacity", "78\n"),
        ("status", "Charging\n"),
    ] {
        let path = battery.join(name);
        fs::write(&path, format!("{value:<4096}")).unwrap();
        assert_eq!(fs::metadata(path).unwrap().len(), 4096);
    }
    let value = archr_ui::status::read_battery(root.path()).unwrap();
    assert_eq!(value.percent, 78);
    assert!(value.charging);
}
