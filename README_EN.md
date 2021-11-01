# How to join DBC network

If you have some DBC and want to get more, you can choose to **become a `validator`**, which requires 7\*24 hours to run a node. If you don't want to run a node, but still want to get profit, you can choose to be a **nominator by nominate `validators`**.

- `Validator`: A validator needs to maintain a full node, which is mainly responsible for verifying transactions and generating blocks based on consensus.
- `Nominator (Nominator)`: A nominator needs to stake `DBC` and nominate the `validator`, by this way it is possible to generate the `validator` in the network and share the rewards with the `validator`. DBC will be slash when the validator is punished.

## Introduction

#### Periods of common actions and attributes in DBC network

- Slot：30 seconds **(generally one block per slot)**
- Epoch duration：4 hour
- Era duration：24 hours (6 sessions per Era, one Era is an election peroid, and is also a reward calculation peroid)
- The `n-1` Era election period (the election period interval is 1 Era) will generate a new set of Validators, responsible for `n+1` Era block generation

#### Total reward amount：

- In [0 ～ 3) year: 10^9 DBC every year
- In [3~8) year: 0.5 \* 10^9 DBC every year
- In [8, 8+) year：0.5 _ (2.5 _ 10^9 + DBC_from_renter) / 5 DBC every year

#### Reward rules：

- After each block is generated, the rewards obtained by the block producer will be recorded to the `ErasRewardPoints`, and the rules for reward `EraRewardPoints` are:

  - 20 points for main chain block producers
  - 2 points for uncle block producers
  - 1 point for producers who quoted uncle blocks

- The validators get same amount of rewards for the same job
- **Reward retention time**: **84 era (84 days)**, rewards exceeding the retention time will not be recorded. Anyone can send `Payout` transactions to get rewards (even if they do not participate in the stake), and everyone who staked in the validator node can get rewards by a single call.
- **Validator Reward** = Total Reward _ Customized Commissions + Remaining Part of Reward _ Percentage of validator staked
- **Nominator Award**: **Only the top 128 of stakes can be rewarded ( according to the stake amount).** `Number of rewards = (total rewards of nodes - custom commissions of validators) * Percentage of nominator's stake`

#### How to be a `norminator`

[How to nominate on DBC](docs/staking_dbc_and_voting.md) -- stake DBC，be a Nominator to get rewards

#### How to be a `validator`

[How to be a DBC validator](docs/join_dbc_network_EN.md) -- run a full node，be a Validator to get rewards
