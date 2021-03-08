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

// // TODO: custom wallet deserializer
// pub fn de_vecstring_to_bytes<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     struct VecString(PhantomData<Vec<Vec<u8>>>);

//     impl<'de> alt_serde::de::Visitor<'de> for VecString {
//         type Value = Vec<Vec<u8>>;

//         fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//             formatter.write_str("string or list of strings")
//         }

//         fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
//         where
//             E: Error,
//         {
//             // let s: &str = Deserialize::deserialize(value)?;
//             Ok(vec![value.as_bytes().to_vec()])
//         }

//         fn visit_seq<S>(self, mut visitor: S) -> Result<Self::Value, S::Error>
//         where
//             S: alt_serde::de::SeqAccess<'de>,
//         {
//             let mut wallets = Vec::new();
//             while let Some(value) = visitor.next_element::<&str>()? {
//                 wallets.push(value.as_bytes().to_vec())
//             }
//             Ok(wallets.into())
//             // de::Deserialize::deserialize(de::value::SeqAccessDeserializer::new(visitor))
//         }
//     }

//     deserializer.deserialize_any(VecString(PhantomData))
// }

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
    gpu: GPU,
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

    // #[serde(deserialize_with = "de_string_to_bytes")]
    // #[serde(borrow)]
    wallet: Vec<Vec<u8>>, // FIXME: fix it
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct CPU {
    #[serde(deserialize_with = "de_string_to_bytes")]
    num: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    #[serde(rename = "type")]
    _type: Vec<u8>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct Disk {
    #[serde(deserialize_with = "de_string_to_bytes")]
    size: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    free: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    #[serde(rename = "type")]
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
    #[serde(rename = "type")]
    _type: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    mem: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    pcie_bandwidth: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    mem_bandwidth: Vec<u8>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct GPUStatus {
    gpus: Vec<OneGPUStatus>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct OneGPUStatus {
    #[serde(deserialize_with = "de_string_to_bytes")]
    id: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    state: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    #[serde(rename = "type")]
    _type: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    uuid: Vec<u8>,
}

#[serde(crate = "alt_serde")]
#[derive(Deserialize, Encode, Decode, Default, Debug)]
struct GPUUsage {
    #[serde(deserialize_with = "de_string_to_bytes")]
    gpu: Vec<u8>,
    #[serde(deserialize_with = "de_string_to_bytes")]
    mem: Vec<u8>,
}
