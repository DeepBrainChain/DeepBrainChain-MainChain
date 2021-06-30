# TODO: 初始化设置链参数

# Alice
reporter="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
reporter_key="0xe5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a"
reporter_box_pubkey="0xff3033c763f71bc51f372c1dc5095accc26880e138df84cac13c46bfd7dbd74f"

# Bob
committee="5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
committee_key="0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89"

# node gen_boxpubkey.js --key "0x868020ae0687dda7d57565093a69090211449845a7e11453612800b663307246"

# online一台机器

# report_machine_fault


node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module maintainCommittee --func reportMachineFault \
    --key $reporter_key $hash $reporter_box_pubkey # FIXME: reporter_box_pubkey 为str


# ❯ node seal_and_open.js --sender_key e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a --receiver_key 398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89 --msg "abcdefg gfedcba"
# Sender sends encrypted message to receiver 141,198,49,88,102,164,244,151,157,215,176,13,196,65,37,129,244,244,65,245,49,20,198,31,183,53,2,133,219,158,101, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11
# Opened message is: abcdefg gfedcba
# ❯ node seal_and_open.js --sender_key e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a --receiver_key 398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89 --msg "abcdefg gfedcba"
# Sender sends encrypted message to receiver 141,198,49,88,102,164,244,151,157,215,176,13,196,65,37,129,244,244,65,245,49,20,198,31,183,53,2,133,219,158,101, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11
# Opened message is: abcdefg gfedcba
