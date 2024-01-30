# compare types of assets pallet from v3.0.0 & polkadot-v0.9.39

Storage(v0.9.37):

1. 
```
Asset = StorageMap<T::AssetId, AssetDetails<T::Balance, T::AccountId, DepositBalanceOf<T, I>>>;

其中，AssetDetails：
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetDetails<Balance, AccountId, DepositBalance> {
    /// Can change `owner`, `issuer`, `freezer` and `admin` accounts.
    pub(super) owner: AccountId,
    /// Can mint tokens.
    pub(super) issuer: AccountId,
    /// Can thaw tokens, force transfers and burn tokens from any account.
    pub(super) admin: AccountId,
    /// Can freeze tokens.
    pub(super) freezer: AccountId,
    /// The total supply across all accounts.
    pub(super) supply: Balance,
    /// The balance deposited for this asset. This pays for the data stored here.
    pub(super) deposit: DepositBalance,
    /// The ED for virtual accounts.
    pub(super) min_balance: Balance,
    /// The total number of accounts.
    pub(super) accounts: u32,
    /// The status of the asset
    pub(super) status: AssetStatus,

    
    /// If `true`, then any account with this asset is given a provider reference. Otherwise, it
    /// requires a consumer reference.
    pub(super) is_sufficient: bool,
    /// The total number of accounts for which we have placed a self-sufficient reference.
    pub(super) sufficients: u32,
    /// The total number of approvals.
    pub(super) approvals: u32,
}
```

2. 
```
Account= StorageDoubleMap<T::AssetId, T::AccountId, AssetAccountOf<T, I>>;

其中，AssetAccountOf:
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetAccount<Balance, DepositBalance, Extra> {
    /// The balance.
    pub(super) balance: Balance,
    /// Whether the account is frozen.
    pub(super) is_frozen: bool,
    /// The reason for the existence of the account.
    pub(super) reason: ExistenceReason<DepositBalance>,
    /// Additional "sidecar" data, in case some other pallet wants to use this storage item.
    pub(super) extra: Extra,
}
```

3. 
```
Approvals<T: Config<I>, I: 'static = ()> = StorageNMap<
(T::AssetId,T::AccountId,T::AccountId), Approval<T::Balance, DepositBalanceOf<T, I>>>;

其中，Approval 为：
/// Data concerning an approval.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, MaxEncodedLen, TypeInfo)]
pub struct Approval<Balance, DepositBalance> {
    /// The amount of funds approved for the balance transfer from the owner to some delegated
    /// target.
    pub(super) amount: Balance,
    /// The amount reserved on the owner's account to hold this item in storage.
    pub(super) deposit: DepositBalance,
}
```

4. 
```
Metadata = StorageMap<T::AssetId => AssetMetadata<DepositBalanceOf<T, I>, BoundedVec<u8, T::StringLimit>>>;

其中，AssetMetadata:
#[derive(Clone, Encode, Decode, Eq, PartialEq, Default, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetMetadata<DepositBalance, BoundedString> {
    /// The balance deposited for this metadata.
    ///
    /// This pays for the data stored in this struct.
    pub(super) deposit: DepositBalance,
    /// The user friendly name of this asset. Limited in length by `StringLimit`.
    pub(super) name: BoundedString,
    /// The ticker symbol for this asset. Limited in length by `StringLimit`.
    pub(super) symbol: BoundedString,
    /// The number of decimals this asset uses to represent one unit.
    pub(super) decimals: u8,
    /// Whether the asset metadata may be changed by a non Force origin.
    pub(super) is_frozen: bool,
}
```


1. 
```
Asset<T: Config> = StorageMap<T::AssetId, AssetDetails<T::Balance, T::AccountId, BalanceOf<T>>>;

其中，AssetDetails：
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct AssetDetails<
	Balance: Encode + Decode + Clone + Debug + Eq + PartialEq,
	AccountId: Encode + Decode + Clone + Debug + Eq + PartialEq,
	DepositBalance: Encode + Decode + Clone + Debug + Eq + PartialEq,
> {
	/// Can change `owner`, `issuer`, `freezer` and `admin` accounts.
	owner: AccountId,

	/// Can mint tokens.
	issuer: AccountId,
	/// Can thaw tokens, force transfers and burn tokens from any account.
	admin: AccountId,
	/// Can freeze tokens.
	freezer: AccountId,
	/// The total supply across all accounts.
	supply: Balance,
	/// The balance deposited for this asset.
	///
	/// This pays for the data stored here together with any virtual accounts.
	deposit: DepositBalance,
	/// The ED for virtual accounts.
	min_balance: Balance,
	/// The total number of accounts.
	accounts: u32,
	

    /// The number of balance-holding accounts that this asset may have, excluding those that were
	/// created when they had a system-level ED.
	max_zombies: u32,
	/// The current number of zombie accounts.
	zombies: u32,
	/// Whether the asset is frozen for permissionless transfers.
	is_frozen: bool,
}
```

2. 
```
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default)]
pub struct AssetBalance<
	Balance: Encode + Decode + Clone + Debug + Eq + PartialEq,
> {
	/// The balance.
	balance: Balance,
	/// Whether the account is frozen.
	is_frozen: bool,
	/// Whether the account is a zombie. If not, then it has a reference.
	is_zombie: bool,
}
```

3. 
```
Metadata<T: Config> = StorageMap<T::AssetId, AssetMetadata<BalanceOf<T>>>;

其中，
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default)]
pub struct AssetMetadata<DepositBalance> {
	/// The balance deposited for this metadata.
	///
	/// This pays for the data stored in this struct.
	deposit: DepositBalance,
	/// The user friendly name of this asset. Limited in length by `StringLimit`.
	name: Vec<u8>,
	/// The ticker symbol for this asset. Limited in length by `StringLimit`.
	symbol: Vec<u8>,
	/// The number of decimals this asset uses to represent one unit.
	decimals: u8,
}
```
