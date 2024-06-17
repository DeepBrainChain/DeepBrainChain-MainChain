#!/usr/bin/env python3

from hashlib import blake2b

report_id = "0"
committee_rand_str = "abc1"
is_support = "1"  # 支持举报填1，不支持举报填0

committee_msg = report_id + committee_rand_str + is_support
h2 = blake2b(digest_size=16)
h2.update(committee_msg.encode())

print("## ReportID:\t", report_id)
print("## RandStr:\t", committee_rand_str)
print("## IsSupport:\t", is_support)
print()
print("CommitteeHash: \t0x" + h2.hexdigest())
