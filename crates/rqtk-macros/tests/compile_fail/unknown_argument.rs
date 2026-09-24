use rqtk_macros::verifies;

#[verifies("VA-SYS-001-01", kase = "typo")]
#[test]
fn this_should_not_compile() {}
