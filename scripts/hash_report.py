#!/usr/bin/env python3

from hashlib import blake2b
import json

reporter_info = json.loads(
    """
{
  "machine_id": "166aead3997957ce0e76b9e5fa5b12e2b7bd04a964f267171a2d458300ae7021",
  "reporter_rand_str": "abcdefg1",
  "error_reason": "it's gpu is broken"
}
    """
)

reporter_str = (
    reporter_info["machine_id"]
    + reporter_info["reporter_rand_str"]
    + reporter_info["error_reason"]
)

h = blake2b(digest_size=16)
h.update(reporter_str.encode())
print("###### Reporter Hash: 0x" + h.hexdigest())


# committee_info = json.loads(
#     """
# {
#   "machine_id": "166aead3997957ce0e76b9e5fa5b12e2b7bd04a964f267171a2d458300ae7021",
#   "reporter_rand_str": "abcdefg1",
#   "committee_rand_str": "abcdefg2",
#   "support_report": "1",
#   "error_reason": "it's gpu is broken",
#   "extra_err_info": "It's true, and mem is broken"
# }
#     """
# )

committee_info = reporter_info
committee_info["extra_err_info"] = "It's true, and mem is broken"
committee_info["support_report"] = "1"

for i in [1, 2, 3]:
    committee_info["committee_rand_str"] = "abcdefg_committee" + str(i)
    committee_str = (
        committee_info["machine_id"]
        + committee_info["reporter_rand_str"]
        + committee_info["committee_rand_str"]
        + committee_info["support_report"]
        + committee_info["error_reason"]
        + committee_info["extra_err_info"]
    )
    h = blake2b(digest_size=16)
    h.update(committee_str.encode())
    print("##### Committee" + str(i) + " Hash: 0x" + h.hexdigest())
