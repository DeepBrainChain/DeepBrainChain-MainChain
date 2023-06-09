use super::super::mock::{TerminatingRental as IRMachine, *};
use dbc_support::{
    live_machine::LiveMachine,
    machine_info::MachineInfo,
    machine_type::{Latitude, Longitude, StakerCustomizeInfo},
    verify_online::StashMachine,
};
use frame_support::assert_ok;

#[test]
fn gen_server_room_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::set_controller(RuntimeOrigin::signed(stash), controller));
        assert_ok!(IRMachine::gen_server_room(RuntimeOrigin::signed(controller)));
        assert_ok!(IRMachine::gen_server_room(RuntimeOrigin::signed(controller)));

        let server_rooms = IRMachine::stash_server_rooms(stash);
        assert_eq!(server_rooms.len(), 2);
        assert_eq!(
            Balances::free_balance(&controller),
            INIT_BALANCE - 20 * ONE_DBC - 20000 * ONE_DBC
        );
        // 同时也是committee，需要质押20000
        assert_eq!(Balances::reserved_balance(&controller), 20000 * ONE_DBC);
    })
}

// set_controller
// gen_server_room
// bond_machine
// add_machine_info
#[test]
fn bond_machine_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::set_controller(RuntimeOrigin::signed(stash), controller));
        assert_ok!(IRMachine::gen_server_room(RuntimeOrigin::signed(controller)));

        // Bob pubkey
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let msg = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48\
                   5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL";
        let sig = "b4084f70730b183127e9db78c6d8dcf79039f23466cd1ee8b536c40c3027a83d\
                   ab040be4ed2db57b67eaac406817a69ce72a13f8ac11ba460e15d318b1504481";

        // - Writes: LiveMachine, StashMachines, MachineInfo,
        // - Write: StashStake, Balance
        assert_ok!(IRMachine::bond_machine(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            msg.as_bytes().to_vec(),
            hex::decode(sig).unwrap()
        ));
        {
            assert_eq!(
                IRMachine::live_machines(),
                LiveMachine { bonding_machine: vec![machine_id.clone()], ..Default::default() }
            );
            assert_eq!(
                IRMachine::stash_machines(stash),
                StashMachine { total_machine: vec![machine_id.clone()], ..Default::default() }
            );
            assert_eq!(
                IRMachine::machines_info(machine_id.clone()),
                Some(MachineInfo {
                    machine_stash: stash,
                    bonding_height: 2,
                    stake_amount: 10000 * ONE_DBC,
                    controller,
                    renters: vec![],
                    last_machine_restake: 0,
                    online_height: 0,
                    last_online_height: 0,
                    init_stake_per_gpu: 0,
                    machine_status: Default::default(),
                    total_rented_duration: 0,
                    total_rented_times: 0,
                    total_rent_fee: 0,
                    total_burn_fee: 0,
                    machine_info_detail: Default::default(),
                    reward_committee: vec![],
                    reward_deadline: 0
                })
            );

            assert_eq!(IRMachine::stash_stake(stash), 10000 * ONE_DBC);
            assert_eq!(Balances::reserved_balance(stash), 10000 * ONE_DBC);
        }

        assert_ok!(IRMachine::gen_server_room(RuntimeOrigin::signed(controller)));
        let server_rooms = IRMachine::stash_server_rooms(stash);

        assert_ok!(IRMachine::add_machine_info(
            RuntimeOrigin::signed(controller),
            machine_id.clone(),
            StakerCustomizeInfo {
                server_room: server_rooms[0],
                upload_net: 100,
                download_net: 100,
                longitude: Longitude::East(1157894),
                latitude: Latitude::North(235678),
                telecom_operators: vec!["China Unicom".into()],
            }
        ));
        // - Writes: LiveMachine, MachinesInfo
        {
            assert_eq!(
                IRMachine::live_machines(),
                LiveMachine { confirmed_machine: vec![machine_id.clone()], ..Default::default() }
            );
        }
    })
}
