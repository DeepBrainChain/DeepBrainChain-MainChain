use codec::{Decode, Encode};
#[cfg(feature = "std")]
use dbc_support::rpc_types::serde_text;
use dbc_support::{ItemList, MachineId, RentOrderId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::{vec, vec::Vec};

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MachineGPUOrder {
    // 机器所有GPU对应的RentOrder
    pub rent_order: Vec<RentOrderId>,
    // 机器订单已经使用的gpu:
    pub used_gpu: Vec<u32>,
}

impl MachineGPUOrder {
    // 获取可以被租用的GPU index
    pub fn gen_rentable_gpu(&mut self, need_gpu: u32, total_gpu: u32) -> Vec<u32> {
        let mut out = vec![];

        for i in 0..total_gpu {
            if out.len() == need_gpu as usize {
                return out
            }

            if self.used_gpu.binary_search(&i).is_err() {
                out.push(i);
                ItemList::add_item(&mut self.used_gpu, i);
            }
        }

        out
    }

    // 根据gpu_index清理使用的GPU index
    pub fn clean_expired_order(&mut self, order_id: RentOrderId, gpu_index: Vec<u32>) {
        ItemList::rm_item(&mut self.rent_order, &order_id);
        for index in gpu_index {
            ItemList::rm_item(&mut self.used_gpu, &index);
        }
    }
}
