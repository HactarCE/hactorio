use super::*;

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
fn empty_settings_have_no_rows() {
    assert!(pack_settings(&[], 300.0, 20.0).is_empty());
}
