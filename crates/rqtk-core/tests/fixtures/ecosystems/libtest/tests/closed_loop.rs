// rqtk: verifies VA-RS-05
#[test]
fn settles() {}

// Unrelated and failing, with the same name as a linked unit test in src/lib.rs.
#[test]
fn cut_off() {
    assert_eq!(1 + 1, 3);
}
