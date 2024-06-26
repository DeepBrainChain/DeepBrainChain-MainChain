#!/usr/bin/env python3

from scalecodec.utils.ss58 import ss58_decode
account = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"

account = '0x{}'.format(ss58_decode(account))
print("Public key of account is:", account) # 0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d

# ss58 decode (5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY) -> a
a = [42, 212, 53, 147, 199, 21, 253, 211, 28, 97, 20, 26, 189, 4, 169, 159, 214, 130, 44, 133, 88, 133, 76, 205, 227, 154, 86, 132, 231, 165, 109, 162, 125, 29, 33]
print(''.join(format(x, '02x') for x in a)) # 2ad43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d1d21 
