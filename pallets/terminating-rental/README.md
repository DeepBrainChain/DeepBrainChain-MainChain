# 可打断式租用

## 1. 机器上线

1.1 资金账户(stash)设置控制账户(controller):

`set_controller`

1.2 控制账户生成机房ID

`gen_server_room`

1.3 使用机器私钥对上链信息(机器ID+stash账户)进行签名

1.4 控制账户提交签名信息，此时需要质押10000DBC，将在机器审核通过后退还

`bond_machine`

1.4 控制账户添加机房信息

`add_machine_info`

## 2. 机器上线审核

完成上述步骤后，机器将分派给三个委员会进行独立验证。在验证时间内，委员会可以审核机器的硬件信息，并提交到链上。

2.1 提交硬件信息的Hash

`submit_confirm_hash`

2.2 提交原始硬件信息

`submit_confirm_raw`

## 3. 租用机器

3.1 租用机器

`rent_machine`

3.2 确认租用成功

`confirm_rent`

3.3 续租

`relet_machine`

## 4. 机器管理

4.1 机器下线

`machine_offline`

4.2 机器上线

`machine_online`

4.3 机器退出

`machine_exit`

## 5. 举报

- [ ] 举报
