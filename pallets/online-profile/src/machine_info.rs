use alt_serde::{Deserialize, Deserializer};
use codec::{Decode, Encode};
use sp_std::{prelude::*, str};

pub fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(de)?;
    Ok(s.as_bytes().to_vec())
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
pub struct MachineInfo {
    error_code: u32,
    data: MachineData,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct MachineData {
    cpu: CPU,

    #[serde(deserialize_with = "de_string_to_bytes")]
    cpu_usage: Vec<u8>,

    disk: Disk,
    gpu: u64,
    gpu_state: GPUStatus,
    gpu_usage: Vec<GPUUsage>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    ip: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    mem: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    mem_usage: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    network_dl: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    network_ul: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    os: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    state: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    version: Vec<u8>,

    #[serde(deserialize_with = "de_string_to_bytes")]
    wallet: Vec<u8>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct CPU {
    num: u32,
    #[serde(deserialize_with = "de_string_to_bytes")]
    _type: Vec<u8>, // TODO: how to handle this
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct Disk {
    #[serde(deserialize_with = "de_string_to_bytes")]
    size: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    free: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    _type: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    speed: Vec<u8>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct GPU {
    #[serde(deserialize_with = "de_string_to_bytes")]
    num: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    driver: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    cuda: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    p2p: Vec<u8>,
    gpus: Vec<GPUDetail>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct GPUDetail {
    #[serde(deserialize_with = "de_string_to_bytes")]
    id: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    _type: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    mem: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    price_bandwitdh: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    mem_bandwidth: Vec<u8>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct GPUStatus {}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct GPUUsage {}
