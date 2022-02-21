#!/usr/bin/env python3

from hashlib import blake2b

machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"

# 根据角色（报告人|验证委员会）来修改下面两个变量之一：
# 报告人修改`reporter..`；委员会修改`committee..`
reporter_rand_str = "abcdef"
committee_rand_str = "fedcba"

err_reason = "补充信息，可留空"

reporter_msg = machine_id + reporter_rand_str + err_reason
committee_msg = machine_id + reporter_rand_str + committee_rand_str + "1" + err_reason

h = blake2b(digest_size=16)
h.update(reporter_msg.encode())
print("ReporterHash: 0x" + h.hexdigest())

h2 = blake2b(digest_size=16)
h2.update(committee_msg.encode())
print("CommitteeHash: 0x" + h2.hexdigest())
