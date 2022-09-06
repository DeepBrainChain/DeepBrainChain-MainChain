use super::Runtime;

pub struct OnlineProfileStorageMigration;
impl frame_support::traits::OnRuntimeUpgrade for OnlineProfileStorageMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        online_profile::migrations::apply::<Runtime>()
    }
}
