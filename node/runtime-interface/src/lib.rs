use sp_runtime_interface::runtime_interface;

#[runtime_interface]
pub trait Sandbox {
    fn invoke(_: i32, _: i64, _: i64, _: i32, _: i32, _: i32) -> i32 {
        unimplemented!()
    }

    fn memory_teardown(_: i32) {
        unimplemented!()
    }

    fn memory_new(_: i32, _: i32) -> i32 {
        unimplemented!()
    }

    fn memory_set(_: i32, _: i32, _: i32, _: i32) -> i32 {
        unimplemented!()
    }

    fn memory_get(_: i32, _: i32, _: i32, _: i32) -> i32 {
        unimplemented!()
    }

    fn instance_teardown(_: u32) {
        unimplemented!()
    }

    fn instantiate(_: i32, _: i64, _: i64, _: i32) -> i32 {
        unimplemented!()
    }
}
