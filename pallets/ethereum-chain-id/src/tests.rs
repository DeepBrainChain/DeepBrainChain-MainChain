#![cfg(test)]

use crate::mock::*;
use frame_support::assert_ok;

#[test]
fn set_chain_id_works() {
    new_test_with().execute_with(|| {
        assert_ok!(EthereumChainId::set_chain_id(RawOrigin::Root.into(), 1));
        System::assert_last_event(
			RuntimeEvent::EthereumChainId(crate::Event::SetChainId {
				chain_id: 1
			}
		));
    })
}

