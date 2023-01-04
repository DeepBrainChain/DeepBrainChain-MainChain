use super::super::mock::*;
use crate::mock::{new_test_ext_after_machine_online, run_to_block};
use dbc_support::{
    live_machine::LiveMachine,
    machine_type::{CommitteeUploadInfo, Latitude, Longitude, MachineStatus, StakerCustomizeInfo},
    verify_online::StashMachine,
    MachineId,
};
use frame_support::assert_ok;
use online_profile::PosGPUInfo;
use pallet_balances::AccountData;
use std::convert::TryInto;
use system::AccountInfo;

// 机器被审核通过后，stash账户币不够，主动调用fulfill_machine来补充质押
#[test]
fn fulfill_machine_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
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
            StakerCustomizeInfo {
                // server_room: H256::from_low_u64_be(1),
                server_room: server_room[0],
                upload_net: 10000,
                download_net: 10000,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
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

        let mut committee_upload_info = CommitteeUploadInfo {
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

        run_to_block(13);

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

            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { online_machine: vec![machine_id.clone()], fulfilling_machine: vec![machine_id2.clone()],..Default::default() }
            );
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

            assert_eq!(
                OnlineProfile::live_machines(),
                LiveMachine { online_machine: vec![machine_id.clone(), machine_id2.clone()], ..Default::default() }
            );
        }
    })
}

#[test]
fn reset_controller_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let pre_controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let post_controller = sr25519::Public::from(Sr25519Keyring::Dave);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert_ok!(OnlineProfile::stash_reset_controller(Origin::signed(stash), post_controller));

        // - Writes: controller_machines, stash_controller, controller_stash, machine_info,
        let empty_machine_id: Vec<MachineId> = vec![];
        assert_eq!(OnlineProfile::controller_machines(pre_controller), empty_machine_id);
        assert_eq!(OnlineProfile::controller_machines(post_controller), vec![machine_id.clone()]);

        assert_eq!(OnlineProfile::stash_controller(stash), Some(post_controller));

        assert_eq!(OnlineProfile::controller_stash(pre_controller), None);
        assert_eq!(OnlineProfile::controller_stash(post_controller), Some(stash));

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.controller, post_controller);
    })
}

#[test]
fn galaxy_on_works() {
    new_test_ext_after_machine_online().execute_with(|| {})
}

#[test]
fn machine_exit_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.reward_deadline, 1 + 365 * 2);

        // run_to_block(366 * 2880 + 1);
        // assert_ok!(OnlineProfile::machine_exit(Origin::signed(controller), machine_id.clone()));
        assert_ok!(OnlineProfile::do_machine_exit(machine_id.clone(), machine_info));

        {
            // 确保machine退出后，还能继续领奖励?还是说直接不能领奖励了
            let machine_info = OnlineProfile::machines_info(&machine_id);
            assert_eq!(machine_info.stake_amount, 0);
            assert_eq!(machine_info.machine_status, MachineStatus::Exit);

            // PosGPUInfo已经被清空
            assert!(!PosGPUInfo::<TestRuntime>::contains_key(
                machine_info.longitude(),
                machine_info.latitude()
            ));
            // 从live_machine中被删除

            // 从controller_machines中删除
            assert!(OnlineProfile::controller_machines(&controller)
                .binary_search(&machine_id)
                .is_err());

            // ErasMachinePoints不应该再存在该变量

            // stash_machines中删除
            let stash_machines = OnlineProfile::stash_machines(&stash);
            assert_eq!(
                stash_machines,
                StashMachine {
                    // 因为total_rent_fee为0， total_burn_fee为0
                    ..Default::default()
                }
            );

            // 确保质押是否变化
            assert_eq!(OnlineProfile::sys_info(), online_profile::SysInfoDetail::default());

            assert_eq!(OnlineProfile::stash_stake(&stash), 0);

            assert_eq!(Balances::free_balance(&stash), INIT_BALANCE);
            assert_eq!(Balances::reserved_balance(&stash), 0);
        }
    })
}

#[test]
fn restake_online_machine_works() {}

#[test]
fn cancel_online_profile_slash_works() {}
