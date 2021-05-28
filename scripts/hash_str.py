#!/usr/bin/env python3

# example: str blake2_128 hash 

from hashlib import blake2b
h = blake2b(digest_size=16)
h.update(b'Hello world!')
print("0x" + h.hexdigest())

