#!/usr/bin/env python3

from hashlib import blake2b
import json

raw_info = json.loads(
    """
{
  "machine_id": "166aead3997957ce0e76b9e5fa5b12e2b7bd04a964f267171a2d458300ae7021",
  "reporter_rand_str": "abcdefg1",
  "error_reason": "it's gpu is broken",
}
    """
)

raw_info2 = json.loads(
    """
{
  "machine_id": "166aead3997957ce0e76b9e5fa5b12e2b7bd04a964f267171a2d458300ae7021",
  "reporter_rand_str": "abcdefg1",
  "committee_rand_str": "abcdefg2",
  "support_report": "1",
  "error_reason": "it's gpu is broken",
  "extra_err_info": "It's true, and mem is broken",
}
    """
)

raw_input0 = (
    raw_info["machine_id"] + raw_info["report_rand_str"] + raw_info["error_reason"]
)


raw_input1 = (
    raw_info["machine_id"]
    + raw_info["reporter_rand_str"]
    + raw_info["committee_rand_str"]
    + raw_info["support_report"]
    + +raw_info["error_reason"]
    + +raw_info["extra_err_info"]
)


h = blake2b(digest_size=16)
h.update(raw_input0.encode())
print("0x" + h.hexdigest())

h = blake2b(digest_size=16)
h.update(raw_input1.encode())
print("0x" + h.hexdigest())
