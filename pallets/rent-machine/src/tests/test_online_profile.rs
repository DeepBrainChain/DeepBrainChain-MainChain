use super::super::mock::*;
use crate::mock::{new_test_ext_after_machine_online, run_to_block};
use frame_support::assert_ok;
use online_profile::{EraStashPoints, LiveMachine, StashMachine, SysInfoDetail};
use pallet_balances::AccountData;
use std::convert::TryInto;
use system::AccountInfo;

// 机器被审核通过后，stash账户币不够，主动调用fulfill_machine来补充质押
#[test]
fn fulfill_machine_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        //   Secret seed:       0xd161967810190fcec218b2c263881b50e4becc6a2ba34ebd45760f44ae3e64d3
        //   Public key (hex):  0xf4f223af57780708fcefeaab01c2ee7fed79262173e16ca01a4a78df1c34f44e
        let machine_id2 = "f4f223af57780708fcefeaab01c2ee7fed79262173e16ca01a4a78df1c34f44e".as_bytes().to_vec();
        let msg = "f4f223af57780708fcefeaab01c2ee7fed79262173e16ca01a4a78df1c34f44e\
                5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "26ed9e3a5c13d01e2239f3a2f39a0825c289ffeaf30da99a4d8516252e28bb0b1c4a119a22528c65ee470718f79f59303f0e3bc074aa17d495d69663fe73838e";

        let committee1 = sr25519::Public::from(Sr25519Keyring::One);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Two);
        let committee3 = sr25519::Public::from(Sr25519Keyring::Dave);

        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let server_room = OnlineProfile::stash_server_rooms(&stash);

        // NOTE: stash把币转走，只剩下 200_000 DBC
        assert_ok!(Balances::transfer(Origin::signed(stash), controller, 9_400_000 * ONE_DBC));
        {
            assert_eq!(System::account(stash), AccountInfo{
                nonce: 0,
                providers: 1,
                data: AccountData {
                    free: 200_000 * ONE_DBC,
                    reserved: 400_000 * ONE_DBC,
                    misc_frozen: 0,
                    fee_frozen: 0,
                },
                ..Default::default()
            });
        }

        // controller bond_machine
        assert_ok!(OnlineProfile::bond_machine(
            Origin::signed(controller),
            machine_id2.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));

        // 控制账户添加机器信息
        assert_ok!(OnlineProfile::add_machine_info(
            Origin::signed(controller),
            machine_id2.clone(),
            online_profile::StakerCustomizeInfo {
                // server_room: H256::from_low_u64_be(1),
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: online_profile::Longitude::East(1157894),
                latitude: online_profile::Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));

        run_to_block(12);

        // 委员会提交机器Hash
        let machine_info_hash1: [u8; 16] = hex::decode("b7be9c4e79d42b5593886c71998a3b50").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id2.clone(),
            machine_info_hash1
        ));
        let machine_info_hash2: [u8; 16] = hex::decode("f6d04fe24ef4db6e94f06b17a6c47e10").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id2.clone(),
            machine_info_hash2
        ));
        let machine_info_hash3: [u8; 16] = hex::decode("b250e10e69a298f74568e539c7b16471").unwrap().try_into().unwrap();
        assert_ok!(OnlineCommittee::submit_confirm_hash(
            Origin::signed(committee3),
            machine_id2.clone(),
            machine_info_hash3
        ));

        let mut committee_upload_info = online_profile::CommitteeUploadInfo {
            machine_id: machine_id2.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 4,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 59890,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };

        // 委员会提交原始信息
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee1), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee2), committee_upload_info.clone()));
        committee_upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(OnlineCommittee::submit_confirm_raw(Origin::signed(committee3), committee_upload_info.clone()));

        // TODO: 检查LiveMachine
        {
            // NOTE: stash把币转走，只剩下 200_000 DBC
            assert_eq!(System::account(stash), AccountInfo{
                nonce: 0,
                providers: 1,
                data: AccountData {
                    free: 100_000 * ONE_DBC,
                    reserved: 500_000 * ONE_DBC,
                    misc_frozen: 0,
                    fee_frozen: 0,
                },
                ..Default::default()
            });
        }

        assert_ok!(Balances::transfer(Origin::signed(controller), stash, 400_000 * ONE_DBC));

        let machine_info = OnlineProfile::machines_info(&machine_id2);
        assert_eq!(machine_info.init_stake_per_gpu, 100_000 * ONE_DBC);
        assert_eq!(machine_info.gpu_num(), 4);

        // 调用fulfill_machine
        assert_ok!(OnlineProfile::fulfill_machine(Origin::signed(controller), machine_id2.clone()));
        {
            // NOTE: stash把币转走，只剩下 200_000 DBC
            assert_eq!(System::account(stash), AccountInfo{
                nonce: 0,
                providers: 1,
                data: AccountData {
                    free: 200_000 * ONE_DBC,
                    reserved: 800_000 * ONE_DBC,
                    misc_frozen: 0,
                    fee_frozen: 0,
                },
                ..Default::default()
            });

            // LiveMachine

            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { rented_machine: vec![machine_id2.clone()], ..Default::default() }
            );
        }
    })
}

#[test]
fn galaxy_on_works() {
    new_test_ext_after_machine_online().execute_with(|| {})
}

#[test]
fn machine_exit_works() {}

#[test]
fn restake_online_machine_works() {}

#[test]
fn cancel_online_profile_slash_works() {}
