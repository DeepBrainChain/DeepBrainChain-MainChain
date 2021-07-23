#!/usr/bin/env python3

from hashlib import blake2b
import json

raw_info = json.loads(
    """
{
  "machine_id": "e6a2afe3a00838c7f26e735fdd5fa7e6e9ecb4152dca7ab63fafd082bfaaa262",
  "gpu_type": "GeForceRTX2080Ti",
  "gpu_num": 2,
  "cuda_core": 4352,
  "gpu_mem": 11,
  "calc_point": 13650,
  "sys_disk": 500,
  "data_disk": 72,
  "cpu_type": "AMD EPYC 7302 16-Core Processor",
  "cpu_core_num": 64,
  "cpu_rate": 300,
  "mem_num": 193299,
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
