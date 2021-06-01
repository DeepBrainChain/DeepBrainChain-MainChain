#!/usr/bin/env python3

from hashlib import blake2b

machine_id = "2gfpp3MAB4Aq2ZPEU72neZTVcZkbzDzX96op9d3fvi3"
gpu_type = "GeForceRTX2080Ti"
gpu_num = "4"
cuda_core = "4352"
gpu_mem = "11283456"
calc_point = "6825"
hard_disk = "3905110864"
cpu_type = "Intel(R) Xeon(R) Silver 4110 CPU"
cpu_core_num = "32"
cpu_rate = "26"
mem_num = "527988672"

upload_net = "22948504"  # 不确定的数值
download_net = "30795411"  # 不确定的数值
longitude = "3122222"  # 经度, 不确定值，存储平均值
latitude = "12145806"  # 纬度, 不确定值，存储平均值

rand_str = "abcdefg"
is_support = "true"

raw_input = (
    machine_id
    + gpu_type
    + gpu_num
    + cuda_core
    + gpu_mem
    + calc_point
    + hard_disk
    + cpu_type
    + cpu_core_num
    + cpu_rate
    + mem_num
    + upload_net
    + download_net
    + longitude
    + latitude
    + rand_str
    + is_support
)

h = blake2b(digest_size=16)
h.update(raw_input.encode())
print("0x" + h.hexdigest())
