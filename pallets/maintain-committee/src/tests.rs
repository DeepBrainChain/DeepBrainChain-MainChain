#[test]
fn report_machine_fault_works() {}

#[test]
fn report_machine_offline_works() {}

#[test]
fn report_machine_unrentable_works() {}

// 控制账户报告机器下线
#[test]
fn controller_report_online_machine_offline_should_work() {
    new_test_with_online_machine_online_ext().execute_with(|| {})
}
