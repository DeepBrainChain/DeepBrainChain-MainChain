# 如何成为DBC提名人

1. 生成资金账户（已有资金账户可以略过）

   + 方式1：`polkadot{.js}`浏览器插件（推荐）：

     + Chrome 安装链接：[Chrome web store](https://chrome.google.com/webstore/detail/polkadot{js}-extension/mopnmbcafieddcagagdcbnhejhlodfdd)
     + Firefox 安装链接：[Firefox add-ons](https://addons.mozilla.org/en-US/firefox/addon/polkadot-js-extension/)

     安装完成后，通过浏览器插件生成即可

   + 方式2：通过网页钱包[https://test.dbcwallet.io/#/accounts](https://test.dbcwallet.io/#/accounts) ，点击`账户`--`添加账户`进行生成。

   + 方式3：通过命令行方式生成

     ```bash
     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
     cargo install --force subkey --git https://github.com/paritytech/substrate --version 2.0.1 --locked
     ```

2. 获取一些的DBC。打开[https://test.dbcwallet.io/#/accounts](https://test.dbcwallet.io/#/accounts ), 您将能看到您的账户与余额

   ![image-20210122210826588](staking_dbc_and_voting.assets/image-20210122210826588.png)

3. 提名验证人。

   + 导航到`网络 > 质押 > 账户操作`，点击`提名人`

     ![image-20210122210945889](staking_dbc_and_voting.assets/image-20210122210945889.png)

   + 在弹窗中选择存储账户(stash account)，控制账户(controller account)，并填写`绑定的金额(value bonded)`，点击下一步

     ![image-20210122211057762](staking_dbc_and_voting.assets/image-20210122211057762.png)

   

   + 接下来选择您要提名的验证人，点击左侧的账户，或者在输入框中输入地址，将您要提名的验证人添加到右侧（**您可以提名多个验证人**）。

     ![image-20210122211203371](staking_dbc_and_voting.assets/image-20210122211203371.png)

   + 最后点击`Bond & Nominate`发送交易，完成提名。

4. 查看您提名的结果

   导航到`网络 > 质押 > 账户操作`，您可以看到绑定的DBC数目，与提名的候选人。

   ![image-20210122211537605](staking_dbc_and_voting.assets/image-20210122211537605.png)

5. 第二次提名

   `步骤6`的提名实际上包含了两个步骤：`存储账户`设置`控制账户` 和 `提名验证人`。

   我们想要再次提名，只需要点击下方的`提名`进行提名这一个步骤即可：

   导航到`网络 > 质押 > 账户操作`，在存储账户列表的右边点击`提名`按钮，在弹出的选项中，选择你要提名的验证人。
