use crate::mock::*;
use dbc_support::ONE_DAY;
use frame_support::assert_ok;
pub use frame_system::{self as system, RawOrigin};
use pallet_elections_phragmen::SeatHolder;

pub const MAX_LEN: usize = 64;

#[test]
fn test_get_reward_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        {
            // 初始化price_ocw (0.010$)
            assert_eq!(DBCPriceOCW::avg_price(), None);
            for _ in 0..MAX_LEN {
                DBCPriceOCW::add_price(10_000u64);
            }
            DBCPriceOCW::add_avg_price();
            assert_eq!(DBCPriceOCW::avg_price(), Some(10_000u64));

            let council_rewards = CouncilReward::get_rewards();
            assert_eq!(
                council_rewards,
                vec![500_000 * ONE_DBC, 200_000 * ONE_DBC, 200_000 * ONE_DBC]
            );
        }

        {
            // 初始化price_ocw (0.020$)
            for _ in 0..MAX_LEN {
                DBCPriceOCW::add_price(20_000u64);
            }
            DBCPriceOCW::add_avg_price();
            assert_eq!(DBCPriceOCW::avg_price(), Some(20_000u64));
            let council_rewards = CouncilReward::get_rewards();
            assert_eq!(
                council_rewards,
                vec![250_000 * ONE_DBC, 100_000 * ONE_DBC, 100_000 * ONE_DBC]
            );
        }

        {
            // 初始化price_ocw (0.001$)
            for _ in 0..MAX_LEN {
                DBCPriceOCW::add_price(5_000u64);
            }
            DBCPriceOCW::add_avg_price();
            assert_eq!(DBCPriceOCW::avg_price(), Some(5_000u64));
            let council_rewards = CouncilReward::get_rewards();
            assert_eq!(
                council_rewards,
                vec![600_000 * ONE_DBC, 200_000 * ONE_DBC, 200_000 * ONE_DBC]
            );
        }
    })
}

#[test]
fn test_reward_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 初始化price_ocw (0.020$)
        for _ in 0..MAX_LEN {
            DBCPriceOCW::add_price(20_000u64);
        }
        DBCPriceOCW::add_avg_price();
        assert_eq!(DBCPriceOCW::avg_price(), Some(20_000u64));

        let council1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let council2 = sr25519::Public::from(Sr25519Keyring::Bob);
        let council3 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let council4 = sr25519::Public::from(Sr25519Keyring::Dave);
        let council5 = sr25519::Public::from(Sr25519Keyring::Eve);
        let council6 = sr25519::Public::from(Sr25519Keyring::Ferdie);
        // let council7 = sr25519::Public::from(Sr25519Keyring::One);
        let council8 = sr25519::Public::from(Sr25519Keyring::Two);

        let mut members = vec![
            SeatHolder { who: council1, stake: 1, deposit: 2 },
            SeatHolder { who: council2, stake: 2, deposit: 2 },
            SeatHolder { who: council3, stake: 3, deposit: 2 },
            SeatHolder { who: council4, stake: 4, deposit: 2 },
            SeatHolder { who: council5, stake: 5, deposit: 2 },
            SeatHolder { who: council6, stake: 6, deposit: 2 },
        ];
        let prime = Some(council1);

        let treasury = CouncilReward::treasury();
        if treasury.is_none() {
            // TODO: should set treasury first.
            return
        }
        let treasury = treasury.unwrap();
        // assert_ok!(Balances::set_balance(RawOrigin::Root.into(), treasury, 10000000 * ONE_DBC, 0));
        assert_ok!(Balances::transfer(
            RuntimeOrigin::signed(council8),
            treasury,
            1000_0000 * ONE_DBC
        ));
        assert_eq!(Balances::free_balance(treasury), 1000_0000 * ONE_DBC);

        // 模拟从0 -> 30 Day
        let reward_frequency = 30 * ONE_DAY;
        // NOTE: 议会当选后顺延15天发放奖励
        if 15 * ONE_DAY % reward_frequency == 15 * ONE_DAY {
            CouncilReward::reward_council(prime, &mut members);
        }
        assert_eq!(Balances::free_balance(treasury), 1000_0000 * ONE_DBC - 450_000 * ONE_DBC);
        assert_eq!(Balances::free_balance(council1), 250_000 * ONE_DBC + INIT_BALANCE);
        assert_eq!(Balances::free_balance(council6), 100_000 * ONE_DBC + INIT_BALANCE);
        assert_eq!(Balances::free_balance(council5), 100_000 * ONE_DBC + INIT_BALANCE);
    })
}
