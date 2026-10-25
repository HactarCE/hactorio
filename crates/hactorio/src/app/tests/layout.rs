use super::*;
use egui_kittest::kittest::Queryable;

#[test]
fn settings_pack_as_many_per_row_as_fit() {
    let widths = [100.0, 100.0, 100.0];
    assert_eq!(
        pack_settings(&widths, 320.0, 20.0),
        vec![(0, 2), (2, 3)],
        "two fit, the third starts a new row"
    );
    assert_eq!(pack_settings(&widths, 220.0, 20.0), vec![(0, 2), (2, 3)]);
}

#[test]
fn an_oversized_setting_gets_its_own_row() {
    assert_eq!(pack_settings(&[500.0], 300.0, 20.0), vec![(0, 1)]);
    assert_eq!(
        pack_settings(&[100.0, 500.0], 300.0, 20.0),
        vec![(0, 1), (1, 2)]
    );
}

#[test]
fn item_rate_unit_label_follows_the_time_unit() {
    let mut document = Document::default();
    document.settings.time_unit = hactorio_core::TimeUnit::Minute;
    let (mut harness, dir) = completion_harness("item-rate-unit", document);
    harness.run();

    assert!(harness.query_by_label("item/m").is_some());
    assert!(harness.query_by_label("item/s").is_none());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn empty_settings_have_no_rows() {
    assert!(pack_settings(&[], 300.0, 20.0).is_empty());
}
