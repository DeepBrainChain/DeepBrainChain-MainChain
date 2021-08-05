#!/usr/bin/env python3

from hashlib import blake2b
import json

raw_info = json.loads(
    """
{
  "machine_id": "166aead3997957ce0e76b9e5fa5b12e2b7bd04a964f267171a2d458300ae7021",
  "gpu_type": "GeForceRTX3090",
  "gpu_num": 1,
  "cuda_core": 10496,
  "gpu_mem": 24,
  "calc_point": 11545,
  "sys_disk": 2000,
  "data_disk": 20,
  "cpu_type": "Intel(R) Xeon(R) CPU E5-2678",
  "cpu_core_num": 48,
  "cpu_rate": 250,
  "mem_num": 224000,
  "rand_str": "0x61",
  "is_support": true
}
    """
)

is_support = "1"  # 支持传1，不支持传0

rand_str1 = "abcdefg1"
rand_str2 = "abcdefg2"
rand_str3 = "abcdefg3"

raw_input0 = (
    raw_info["machine_id"]
    + raw_info["gpu_type"]
    + str(raw_info["gpu_num"])
    + str(raw_info["cuda_core"])
    + str(raw_info["gpu_mem"])
    + str(raw_info["calc_point"])
    + str(raw_info["sys_disk"])
    + str(raw_info["data_disk"])
    + str(raw_info["cpu_type"])
    + str(raw_info["cpu_core_num"])
    + str(raw_info["cpu_rate"])
    + str(raw_info["mem_num"])
)

print("############################\t", raw_info["machine_id"])

raw_input1 = raw_input0 + rand_str1 + is_support
h = blake2b(digest_size=16)
h.update(raw_input1.encode())
print("0x" + h.hexdigest())

raw_input2 = raw_input0 + rand_str2 + is_support
h = blake2b(digest_size=16)
h.update(raw_input2.encode())
print("0x" + h.hexdigest())

raw_input3 = raw_input0 + rand_str3 + is_support
h = blake2b(digest_size=16)
h.update(raw_input3.encode())
print("0x" + h.hexdigest())
