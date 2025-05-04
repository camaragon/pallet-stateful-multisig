use std::collections::BTreeSet;

use crate as pallet_multisig;
use frame_support::{
	derive_impl,
	traits::{ConstU128, ConstU16, ConstU32, ConstU64},
	BoundedBTreeSet,
};
use pallet_balances::Call as BalancesCall;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;
pub const DEFAULT_THRESHOLD: u32 = 6;
pub const MAX_MEMBERS: u32 = 10;
pub const MULTISIG_DEPOSIT: u128 = 20;
pub const DEFAULT_EXPIRATION_BLOCKS: u64 = 100;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Multisig: pallet_multisig,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ConstU32<10>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ConstU32<10>;
}

impl pallet_multisig::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type NativeBalance = Balances;
	type RuntimeCall = RuntimeCall;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MaxMembers = ConstU32<MAX_MEMBERS>;
	type DefaultThreshold = ConstU32<DEFAULT_THRESHOLD>;
	type MultisigDeposit = ConstU128<MULTISIG_DEPOSIT>;
	type DefaultExpirationBlocks = ConstU64<DEFAULT_EXPIRATION_BLOCKS>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}

pub fn generate_members() -> BoundedBTreeSet<u64, ConstU32<MAX_MEMBERS>> {
	let members_vec = vec![1, 2, 3];
	let members_set: BTreeSet<u64> = members_vec.into_iter().collect();
	BoundedBTreeSet::try_from(members_set).expect("Should have a valid members set")
}

pub fn call_transfer(dest: u64, value: u128) -> Box<RuntimeCall> {
	Box::new(RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest, value }))
}

pub fn call_delete_multisig(multisig_id: u64) -> Box<RuntimeCall> {
	Box::new(RuntimeCall::Multisig(pallet_multisig::Call::delete_multisig { multisig_id }))
}

pub fn call_cancel_transaction(multisig_id: u64, transaction_id: H256) -> Box<RuntimeCall> {
	Box::new(RuntimeCall::Multisig(pallet_multisig::Call::cancel_transaction {
		multisig_id,
		transaction_id,
	}))
}
