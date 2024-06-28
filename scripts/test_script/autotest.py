#!/usr/bin/env python3

import os
import json

def execCmd(cmd):
    r = os.popen(cmd)
    result = r.read()
    r.close()
    return result

# 获取计算机MAC地址和IP地址
if __name__ == '__main__':
    cmd_dict = dict()
    cmd_dict["gen_keypair"] = "subkey generate --scheme sr25519 --output-type Json"
    cmd_dict["transfer"] = ""
    cmd_dict["root_tx"] = ""
    cmd_dict["user_tx"] = ""


    result = execCmd(cmd);
    print(json.loads(result)["secretSeed"])
