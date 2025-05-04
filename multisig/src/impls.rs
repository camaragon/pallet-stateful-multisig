#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_core::blake2_256;
use sp_runtime::{
	traits::{Saturating, TrailingZeroInput},
	BoundedBTreeMap,
};
use sp_std::prelude::*;

use super::*;

impl<T: Config> Pallet<T> {
	/// Derive a unique account id for the multisig.
	pub fn generate_multi_account_id(nonce: u64) -> T::AccountId {
		let entropy = (b"pba/multisig", nonce).using_encoded(blake2_256);
		Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
			.expect("infinite length input; no invalid inputs for type; qed")
	}
	pub fn generate_transaction_id(
		proposer: T::AccountId,
		block_number: BlockNumberFor<T>,
		call_hash: [u8; 32],
	) -> T::Hash {
		let entropy =
			(b"pba/transaction", proposer, block_number, call_hash).using_encoded(blake2_256);
		Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
			.expect("infinite length input; no invalid inputs for type; qed")
	}
	/// Tally the "approved" and "rejected" votes on a proposed transaction.
	pub fn do_tally_votes(
		status: TransactionStatus,
		votes: BoundedBTreeMap<T::AccountId, Vote, T::MaxMembers>,
	) -> Result<(u32, u32), Error<T>> {
		// Ensure the transaction has a "Pending" status
		ensure!(status == TransactionStatus::Pending, Error::<T>::TransactionNotPending);
		// Accumulate the number of approval and rejection votes
		let (approvals, rejections) = votes.values().fold((0, 0), |(a, r), vote| match vote {
			Vote::Approve => (a + 1, r),
			Vote::Reject => (a, r + 1),
		});
		Ok((approvals, rejections))
	}
	/// Build and store a proposed transaction.
	pub fn build_transaction(
		from: T::AccountId,
		multisig_id: T::AccountId,
		call: Box<<T as Config>::RuntimeCall>,
		call_hash: [u8; 32],
	) -> Result<(), Error<T>> {
		let transaction_id = Self::generate_transaction_id(
			from.clone(),
			frame_system::Pallet::<T>::block_number(),
			call_hash,
		);
		let mut votes = BoundedBTreeMap::new();
		votes
			.try_insert(from.clone(), Vote::Approve)
			.map_err(|_| Error::<T>::VoteLimitReached)?;
		let transaction = Transaction {
			proposer: from.clone(),
			call,
			call_hash,
			status: TransactionStatus::Pending,
			votes,
			created_at: frame_system::Pallet::<T>::block_number(),
			// Set the expiration block to the current block number plus the default expiration
			// blocks count
			expires_at: frame_system::Pallet::<T>::block_number()
				.saturating_add(T::DefaultExpirationBlocks::get()),
		};
		Transactions::<T>::insert(&multisig_id, &transaction_id, transaction);
		Self::deposit_event(Event::TransactionCreated {
			proposer: from,
			transaction: transaction_id,
			multisig: multisig_id,
			status: TransactionStatus::Pending,
			call_hash,
		});
		Ok(())
	}
}
