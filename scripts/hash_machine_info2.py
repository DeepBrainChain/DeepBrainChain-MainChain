#!/usr/bin/env python3

from hashlib import blake2b
import json
import sys

# NOTE: cpu_rate: 单位Mhz; sys_disk/data_disk单位: G， mem_num: G
# NOTE: is_support: 支持传1，不支持传0
# NOTE: 请先修改自己的随机字符串: rand_str

with open(sys.argv[1], "r") as fin:
    for aline in fin:
        all_fields = aline.strip().split("\t")
        aline = "".join(aline)
        new_line = aline + "abcdefg1" + "1"

        print("MachineId:\t", all_fields[0])
        h = blake2b(digest_size=16)
        h.update(new_line.encode())
        print("Hash:" + "\t0x" + h.hexdigest())

        # print("aline: ", aline + "abcdefg1" + "1")
        # print("aline[0]", aline[0])
