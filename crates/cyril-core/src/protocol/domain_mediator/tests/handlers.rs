use super::super::DomainWork;

#[test]
fn domain_work_payloads_cross_send_static_sdk_handlers() {
    fn assert_send_static<T: Send + 'static>() {}
    assert_send_static::<DomainWork>();
}
