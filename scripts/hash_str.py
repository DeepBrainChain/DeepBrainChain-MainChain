#!/usr/bin/env python3

from hashlib import blake2b

machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
gpu_type = "GeForceRTX2080Ti"
gpu_num = "4"
cuda_core = "4352"
gpu_mem = "11283456"
calc_point = "6825" # NOTE: 显卡的总算力点数
sys_disk = "12345465"
data_disk = "324567733"
cpu_type = "Intel(R) Xeon(R) Silver 4110 CPU"
cpu_core_num = "32"
cpu_rate = "26"
mem_num = "527988672"
rand_str = "abcdefg"
is_support = "1" # 支持传1，不支持传0

raw_input = (
    machine_id
    + gpu_type
    + gpu_num
    + cuda_core
    + gpu_mem
    + calc_point
    + sys_disk
    + data_disk
    + cpu_type
    + cpu_core_num
    + cpu_rate
    + mem_num
    + rand_str
    + is_support
)

h = blake2b(digest_size=16)
h.update(raw_input.encode())
print("0x" + h.hexdigest())


# # 不确定的值由矿工进行设置
# upload_net = "22948504"  # 不确定的数值
# download_net = "30795411"  # 不确定的数值
# longitude = "3122222"  # 经度, 不确定值，存储平均值
# latitude = "12145806"  # 纬度, 不确定值，存储平均值
