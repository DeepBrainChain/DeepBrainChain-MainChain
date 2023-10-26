# 可打断式租用

> 可打断式租用模块，机器上链方式与[长租模式](https://deepbrainchain.github.io/DBC-Wiki/onchain-guide/bonding-machine.html)机器上链方式类似。
> 请先阅读[长租模式](https://deepbrainchain.github.io/DBC-Wiki/onchain-guide/bonding-machine.html)文档，了解上链的步骤。
> 
> 注意：下面所有的操作都在`terminatingRental`模块进行，而非长租模式的`onlineProfile`模块！

下面机器上链在[DBC网页钱包](https://www.dbcwallet.io/)进行操作。

## 1. 机器上线

1.1 资金账户(stash)设置控制账户(controller):

点击开发者--交易，选择terminatingRental模块的`setController`方法，分别选择资金账户和控制账户，点击右下角"提交交易"，进行绑定。

1.2 控制账户生成机房ID

资金账户第一次绑定机器或者机器在新的机房时，需要生成一个新的机房 ID。每次生成机房支付手续费10 DBC。

如果将要绑定的机器在同一机房，生成一次机房信息即可。如果机器在不同的机房，按情况生成对应个数机房，添加机器信息时，选择对应机房（如果机器在同一机房，则每次添加机器信息，添加相同的机房 ID 即可）。

点击开发者--交易，选择`terminatingRental`模块的`genServerRoom`方法，分别选择资金账户和控制账户，点击右下角"提交交易"，进行绑定。

1.3 使用机器私钥对上链信息(机器ID+stash账户)进行签名

该步骤与长租模式生成机器签名信息完全一致。请参考：[机器生成签名消息](https://deepbrainchain.github.io/DBC-Wiki/onchain-guide/bonding-machine.html#_2-机器生成签名消息)

1.4 控制账户提交签名信息，此时需要质押10000DBC，将在机器审核通过后退还

导航到：开发者--交易，如选择`terminatingRental`模块的`bondMachine`方法。

使用控制账户，将机器ID(MachineId)与控制账户进行绑定即可。参数填写方式，请参考：[使用控制账户将机器绑定到资金账户](https://deepbrainchain.github.io/DBC-Wiki/onchain-guide/bonding-machine.html#_2-3-使用控制账户将机器绑定到资金账户)

1.5 查询机房信息

导航到开发者--链状态，通过资金账户，查看资金账户下绑定的机房：

选择模块`terminatingRental`--`stashServerRooms` 进行查询，参数选择上面的资金账户。

1.6 控制账户添加机房信息

使用控制账户添加机器信息。 导航到开发者--交易--`terminatingRental`--`addMachineInfo`

参数的填写，请参考 [控制账户添加机器信息](https://deepbrainchain.github.io/DBC-Wiki/onchain-guide/bonding-machine.html#_4-控制账户添加机器信息)

至此，机器上链的操作已经完成。请等待审核委员会对机器进行验证审核。

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
