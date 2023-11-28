use micro_rdk_macros::DoCommand;

pub trait DoCommand {
    fn do_command(&self) -> u8 {
        1
    }
}

pub trait TestTrait: DoCommand {
    fn test_fn(&self) -> u8;
}

#[derive(DoCommand)]
pub struct TestStruct {}

impl TestTrait for TestStruct {
    fn test_fn(&self) -> u8 {
        2
    }
}

#[test]
fn do_command_derive() {
    let a = TestStruct {};
    assert_eq!(a.do_command(), 1);
    assert_eq!(a.test_fn(), 2);
}
