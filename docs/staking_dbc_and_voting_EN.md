# How to nominate on DBC?

1. Generate stash account （If you already have stash account, you can skip this）

   + Option 1: Install `polkadot{.js}` adds-on

     + Chrome, install via [Chrome web store](https://chrome.google.com/webstore/detail/polkadot{js}-extension/mopnmbcafieddcagagdcbnhejhlodfdd)
     + Firefox, install via [Firefox add-ons](https://addons.mozilla.org/en-US/firefox/addon/polkadot-js-extension/)

     Then generate by `polkadot{.js}`

   + Option 2: Generate account from [https://test.dbcwallet.io/#/accounts](https://test.dbcwallet.io/#/accounts)，click `Account` -- `Add account`

   + Option 3: Generate by command line:

     ```bash
     curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
     cargo install --force subkey --git https://github.com/paritytech/substrate --version 2.0.1 --locked
     ```

2. Get some DBC. Open [https://test.dbcwallet.io/#/accounts](https://test.dbcwallet.io/#/accounts) and you can see your account and your balance:

   ![image-20210122210826588](staking_dbc_and_voting.assets/image-20210122210826588.png)

3. Nominator

   + Navigate to `Network > Staking > Account actions`, click `Nominator`

     ![image-20210122210945889](staking_dbc_and_voting.assets/image-20210122210945889.png)

   + set your stash account，controller account and`value bonded`，then click next![image-20210122211057762](staking_dbc_and_voting.assets/image-20210122211057762.png)

   + then, you should select the validator. (**You can nominate more than one validator**).![image-20210122211203371](staking_dbc_and_voting.assets/image-20210122211203371.png)

   + Finally click `Bond & Nominate` to send the transaction and finished the nominate.

4. Check your nominate result

   Navigate to`Network > Staking > Account actions`, you can see the balance of bonded DBC and the validator you nominated.

   ![image-20210122211537605](staking_dbc_and_voting.assets/image-20210122211537605.png)

5. Nominate the second time

   Nominate in `step 6` include two step: set `stash account` bond `controller` account and nominate a validator.

   If we want to nominate the second time, only need to click `Nominate` button：

   Navigate to `Staking > Account actions > stashes > Nominate`, select the validator you want to nominate. It's done!

