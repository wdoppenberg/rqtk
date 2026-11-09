// The order service's tests. Not compiled: the end-to-end test writes their JUnit results.

#[verifies("VA-SYS-0001-01")]
#[test]
fn get_order_includes_the_customer() {}

#[verifies("VA-SYS-0002-01")]
#[test]
fn subscribers_receive_order_placed() {}
