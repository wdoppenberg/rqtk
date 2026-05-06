use rqtk_macros::verifies;

#[verifies("VA-DOES-NOT-EXIST")]
#[test]
fn this_should_not_compile() {}
