use super::super::mock::*;

// 情景1: 机器因修改配置而下线，重新上线后，将需要补充质押
#[test]
fn fulfill_should_works() {
    new_test_with_online_machine_distribution().execute_with(|| {})
}

// 情景2：机器初始质押不足，审核通过后，将需要补充质押
#[test]
fn fullfill_should_works2() {
    new_test_with_online_machine_distribution().execute_with(|| {})
}
