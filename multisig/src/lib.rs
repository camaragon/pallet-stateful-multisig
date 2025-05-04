//! # Multisig Pallet
//! A pallet for performing multisig transactions.
//!
//! ## Overview
//!
//! A stateful multi-signature substrate runtime pallet allowing members to create a shared account
//! holding funds. Dispatch calls can be performed on behalf of the multisig account. Each call is
//! tied to a proposed transaction. Each proposed transaction will be voted upon for whether the
//! call will be exceuted or rejected. A proposed transaction can also be canceled. The ability to
//! delete a multisig account is also provided.
//!
//! ### Dispatchable Functions
//!
//! * `create_multisig` - Create a new multisig account with a set of members and an approval/rejection threshold.
//!   The creator must be one of the provided members and must provide a deposit.
//!
//! * `propose_transaction` - Propose a transaction to be executed by the multisig account. Only members
//!   of the multisig group can propose, and the transaction is stored on-chain until it receives enough approvals/rejections.
//!
//! * `fund_multisig` - Fund the multisig account. Anyone can fund the multisig account without
//!   being a member.
//!
//! * `vote` - Submit a vote (approve or reject) for a proposed transaction. Only multisig members can vote.
//!
//! * `submit_transaction` - Submit and execute the transaction once it has reached the required number
//!   of approvals. The proposed transaction can also be canceled if it has enough rejection votes when submitted.
//!
//! * `cancel_transaction` - Cancel a proposed transaction. To be sent via dispatch call on propose
//! transaction only.
//!
//! * `delete_multisig` - Delete a multisig account. To be sent via dispatch call on propose
//! transaction only.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
mod impls;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use frame_support::{
		dispatch::{DispatchResult, GetDispatchInfo, RawOrigin},
		pallet_prelude::{ValueQuery, *},
		traits::{
			fungible::{self, hold::Mutate as HoldMutate, Inspect, Mutate},
			tokens::{Fortitude, Precision, Preservation},
		},
	};
	use frame_system::pallet_prelude::*;
	use sp_core::blake2_256;
	use sp_runtime::{traits::Dispatchable, BoundedBTreeMap, BoundedBTreeSet, Saturating};
	use sp_std::prelude::*;

	pub type BalanceOf<T> = <<T as Config>::NativeBalance as fungible::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The event type of the runtime as a whole.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Type accessing the Balances Pallet.
		type NativeBalance: fungible::Inspect<Self::AccountId>
			+ fungible::Mutate<Self::AccountId>
			+ fungible::hold::Inspect<Self::AccountId>
			+ fungible::hold::Mutate<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ fungible::freeze::Inspect<Self::AccountId>
			+ fungible::freeze::Mutate<Self::AccountId>;

		/// A type representing all available calls in the runtime.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo;

		/// The reason for holding funds in the multisig account.
		type RuntimeHoldReason: From<HoldReason>;

		/// The default constant maximum number of members allowed in a multisig.
		#[pallet::constant]
		type MaxMembers: Get<u32>;

		/// The default constant threshold for number of members required to approve a transaction.
		#[pallet::constant]
		type DefaultThreshold: Get<u32>;

		/// The default constant deposit required to create a multisig.
		#[pallet::constant]
		type MultisigDeposit: Get<BalanceOf<Self>>;

		/// The default constant of exipration blocks for a transaction;
		#[pallet::constant]
		type DefaultExpirationBlocks: Get<BlockNumberFor<Self>>;
	}

	/// Reasons for placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		#[codec(index = 0)]
		MultisigCreationDeposit,
	}

	/// Voting options on a proposed transaction.
	#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, PartialEq)]
	pub enum Vote {
		Approve,
		Reject,
	}

	/// Potential statuses a transaction can have.
	#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, PartialEq)]
	pub enum TransactionStatus {
		Pending,
		Complete,
		Canceled,
		Rejected,
		Expired,
	}

	#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxMembers))]
	pub struct MultisigAccount<AccountId, MaxMembers, BlockNumber> {
		/// The creator of the multisig.
		pub creator: AccountId,
		/// The members of the multisig.
		pub members: BoundedBTreeSet<AccountId, MaxMembers>,
		/// The number of members required to approve a transaction.
		pub threshold: u32,
		/// The block number at which the multisig was created.
		pub created_at: BlockNumber,
	}

	#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxMembers))]
	pub struct Transaction<AccountId, RuntimeCall, MaxMembers, BlockNumber> {
		/// The proposer of the transaction.
		pub proposer: AccountId,
		/// The status of the transaction.
		pub status: TransactionStatus,
		/// The call to be executed.
		pub call: RuntimeCall,
		/// The hash of the call.
		pub call_hash: [u8; 32],
		/// The number of votes proposed on a transaction.
		pub votes: BoundedBTreeMap<AccountId, Vote, MaxMembers>,
		/// The block number at which the transaction was created.
		pub created_at: BlockNumber,
		/// The block number at which the transaction was approved.
		pub expires_at: BlockNumber,
	}

	/// The set of multisigs in storage.
	#[pallet::storage]
	pub type Multisigs<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		MultisigAccount<T::AccountId, T::MaxMembers, BlockNumberFor<T>>,
	>;

	/// The nonce for multisig account generation.
	#[pallet::storage]
	pub type MultisigNonce<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// The set of transactions tied to the corresponding multisig account in storage.
	#[pallet::storage]
	pub type Transactions<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::Hash,
		Transaction<
			T::AccountId,
			Box<<T as Config>::RuntimeCall>,
			T::MaxMembers,
			BlockNumberFor<T>,
		>,
	>;

	/// Pallets use events to inform users when important changes are made.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new mutlisig has been created.
		NewMultisig { creator: T::AccountId, multisig: T::AccountId },
		/// A multisig has been deleted.
		MultisigDeleted { from: T::AccountId, multisig: T::AccountId },
		/// A multisig has been funded.
		MultisigFunded { from: T::AccountId, to: T::AccountId, amount: BalanceOf<T> },
		/// A proposed transaction has been created.
		TransactionCreated {
			proposer: T::AccountId,
			transaction: T::Hash,
			multisig: T::AccountId,
			status: TransactionStatus,
			call_hash: [u8; 32],
		},
		/// A proposed transaction has been voted on.
		TransactionVoted {
			voter: T::AccountId,
			transaction: T::Hash,
			multisig: T::AccountId,
			vote: Vote,
			call_hash: [u8; 32],
		},
		/// A proposed transaction has been submitted.
		TransactionExecuted {
			submitter: T::AccountId,
			transaction: T::Hash,
			multisig: T::AccountId,
			approvals: u32,
			rejections: u32,
			status: TransactionStatus,
			call_hash: [u8; 32],
		},
		/// A proposed transaction has been canceled.
		TransactionCanceled {
			submitter: T::AccountId,
			transaction: T::Hash,
			multisig: T::AccountId,
			status: TransactionStatus,
			call_hash: [u8; 32],
		},
	}

	/// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Already voted.
		AlreadyVoted,
		/// The approval threshold has been reached.
		ApprovalThresholdMet,
		/// Creator must be a member of the multisig.
		ProposerMustBeMember,
		/// Threshold is too high compared to members.
		ThresholdTooHigh,
		/// Multisig does not exist.
		MultisigDoesNotExist,
		/// Transaction already exists.
		TransactionAlreadyExists,
		/// Transaction does not exist.
		TransactionDoesNotExist,
		/// Transaction not pending.
		TransactionNotPending,
		/// Transaction failed.
		TransactionFailed,
		/// Fund transfer failed.
		TransferFailed,
		/// Insufficient funds.
		NotEnoughFunds,
		/// Zero amount.
		ZeroAmount,
		/// Maximum vote limit reached.
		VoteLimitReached,
		/// Not a member of the multisig.
		NotAMember,
		/// The approval threshold has not been reached.
		ThresholdNotReached,
		/// Call hash does not match the expected.
		MismatchingCallHash,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dispatch call function that creates a new multisig account. It requires the creator to
		/// be a member, the threshold must be less than or equal to the number of members, and a
		/// configurable deposit is required. The deposit will become a "Hold" and be returned to
		/// the creator of the multisig in the instance of deletion.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::default())]
		pub fn create_multisig(
			origin: OriginFor<T>,
			members: BoundedBTreeSet<T::AccountId, T::MaxMembers>,
			threshold: Option<u32>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// Ensure the creator is a member of the multisig
			ensure!(members.contains(&who), Error::<T>::ProposerMustBeMember);
			// Ensure the threshold is not too low
			ensure!(
				threshold.unwrap_or(T::DefaultThreshold::get()) <= members.len() as u32,
				Error::<T>::ThresholdTooHigh
			);
			let deposit = T::MultisigDeposit::get();
			// Ensure the signer has enough balance to create the multisig
			ensure!(
				T::NativeBalance::reducible_balance(
					&who,
					Preservation::Preserve,
					Fortitude::Polite
				) >= deposit,
				Error::<T>::NotEnoughFunds
			);
			let nonce = MultisigNonce::<T>::get();
			// Increment the multisig nonce
			MultisigNonce::<T>::put(nonce + 1);
			let multisig_id = Self::generate_multi_account_id(nonce);
			// Use the passed threshold or the default
			let threshold = threshold.unwrap_or(T::DefaultThreshold::get());
			let multisig = MultisigAccount {
				creator: who.clone(),
				members,
				threshold,
				created_at: frame_system::Pallet::<T>::block_number(),
			};
			Multisigs::<T>::insert(&multisig_id, multisig);
			// Transfer to multisig account add 1 to the deposit to cover the transfer fee
			let total_deposit: BalanceOf<T> = deposit.saturating_add(1u32.into());
			T::NativeBalance::transfer(
				&who,
				&multisig_id,
				total_deposit,
				Preservation::Expendable,
			)?;
			// Hold that amount in the multisig account as a "deposit"
			T::NativeBalance::hold(
				&HoldReason::MultisigCreationDeposit.into(),
				&multisig_id,
				deposit,
			)?;

			Self::deposit_event(Event::NewMultisig { creator: who.clone(), multisig: multisig_id });

			Ok(())
		}
		/// Dispatch call function the intentionally allows anyone to fund the multisig account
		/// without having to be a member in the spirit of third pary funding or grants. No vote on
		/// behalf of the multisig is required for this call.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::default())]
		pub fn fund_multisig(
			origin: OriginFor<T>,
			multisig_id: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			// Ensure the fund amount is not zero
			ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
			let who = ensure_signed(origin)?;
			// Ensure the origin has enough balance to fund the multisig
			ensure!(
				T::NativeBalance::reducible_balance(
					&who,
					Preservation::Preserve,
					Fortitude::Polite
				) >= amount,
				Error::<T>::NotEnoughFunds
			);
			let multisig =
				Multisigs::<T>::get(&multisig_id).ok_or(Error::<T>::MultisigDoesNotExist)?;
			// Transfer the funds to the multisig account
			T::NativeBalance::transfer(&who, &multisig_id, amount, Preservation::Preserve)?;
			// Add the new mulisig account to the mulisig storage
			Multisigs::<T>::insert(&multisig_id, multisig);
			Self::deposit_event(Event::MultisigFunded { from: who, to: multisig_id, amount });
			Ok(())
		}
		/// Dispatch call function that proposes a transaction representing a call to be
		/// dispatched. This call will be up for voting and depending on the results of the vote it
		/// will wither be dispatched or rejected.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::default())]
		pub fn propose_transaction(
			origin: OriginFor<T>,
			multisig_id: T::AccountId,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let multisig =
				Multisigs::<T>::get(&multisig_id).ok_or(Error::<T>::MultisigDoesNotExist)?;
			// Ensure the proposer is a member of the multisig
			ensure!(multisig.members.contains(&who), Error::<T>::ProposerMustBeMember);
			let call_hash = blake2_256(&call.encode());
			// Build and store the transaction
			Self::build_transaction(who, multisig_id, call, call_hash)?;
			Ok(())
		}
		/// Dispatch call function that allows a member of the multisig to vote either "Approve" or
		/// "Reject" on the dispatch/submisison of a proposed transaction.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::default())]
		pub fn vote(
			origin: OriginFor<T>,
			multisig_id: T::AccountId,
			transaction_id: T::Hash,
			vote: Vote,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let multisig =
				Multisigs::<T>::get(&multisig_id).ok_or(Error::<T>::MultisigDoesNotExist)?;
			// Ensure the proposer is a member of the multisig
			ensure!(multisig.members.contains(&who), Error::<T>::NotAMember);
			Transactions::<T>::try_mutate(
				&multisig_id,
				&transaction_id,
				|maybe_transaction| -> Result<(), Error<T>> {
					let transaction =
						maybe_transaction.as_mut().ok_or(Error::<T>::TransactionDoesNotExist)?;
					// Ensure the transaction has a "Pending" status
					ensure!(
						transaction.status == TransactionStatus::Pending,
						Error::<T>::TransactionNotPending
					);
					// Ensure the transaction has not already been voted on by the proposer
					ensure!(!transaction.votes.contains_key(&who), Error::<T>::AlreadyVoted);
					// Update the transaction with the new vote
					transaction
						.votes
						.try_insert(who.clone(), vote.clone())
						.map_err(|_| Error::<T>::VoteLimitReached)?;
					Self::deposit_event(Event::TransactionVoted {
						voter: who,
						transaction: transaction_id,
						multisig: multisig_id.clone(),
						vote,
						call_hash: transaction.call_hash,
					});
					Ok(())
				},
			)?;
			Ok(())
		}
		/// Dispatch call function that allows a member of the multisig to attempt to submit a
		/// proposed transaction. Depending on the results of the vote, the call will either be
		/// dispatched, the call will be rejected or the call will return nothing if no threshold
		/// has been broken yet. Both approval and rejection paths will result in the transaction
		/// being removed from storage.
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::default())]
		pub fn submit_transaction(
			origin: OriginFor<T>,
			multisig_id: T::AccountId,
			transaction_id: T::Hash,
			call: Box<<T as Config>::RuntimeCall>,
			call_hash: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let multisig =
				Multisigs::<T>::get(&multisig_id).ok_or(Error::<T>::MultisigDoesNotExist)?;
			// Ensure the proposer is a member of the multisig
			ensure!(multisig.members.contains(&who), Error::<T>::NotAMember);
			// Ensure the trnsaction call hash matches the expected hash
			ensure!(blake2_256(&call.encode()) == call_hash, Error::<T>::MismatchingCallHash);
			let transaction = Transactions::<T>::get(&multisig_id, &transaction_id)
				.ok_or(Error::<T>::TransactionDoesNotExist)?;
			// Ensure the transaction has a "Pending" status
			ensure!(
				transaction.status == TransactionStatus::Pending,
				Error::<T>::TransactionNotPending
			);
			let (approvals, rejections) =
				Self::do_tally_votes(transaction.status.clone(), transaction.votes)?;
			if approvals >= multisig.threshold {
				let res =
					call.clone().dispatch(RawOrigin::Signed(transaction.proposer.clone()).into());
				res.map(|_| ()).map_err(|_e| Error::<T>::TransactionFailed)?;
				Transactions::<T>::remove(&multisig_id, &transaction_id);
				Self::deposit_event(Event::TransactionExecuted {
					submitter: who.clone(),
					transaction: transaction_id,
					multisig: multisig_id.clone(),
					approvals,
					rejections,
					status: TransactionStatus::Complete,
					call_hash,
				});
			}
			if rejections >= multisig.threshold {
				let res = call.dispatch(RawOrigin::Signed(transaction.proposer.clone()).into());
				res.map(|_| ()).map_err(|_e| Error::<T>::TransactionFailed)?;
				Transactions::<T>::remove(&multisig_id, &transaction_id);
				Self::deposit_event(Event::TransactionExecuted {
					submitter: who,
					transaction: transaction_id,
					multisig: multisig_id,
					approvals,
					rejections,
					status: TransactionStatus::Complete,
					call_hash,
				});
			}
			Ok(())
		}
		/// WARNING: Only meant to be executed via propose transaction call dispatch.
		/// Dispatch funciton call to propose canceling an existing proposed transaction.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::default())]
		pub fn cancel_transaction(
			origin: OriginFor<T>,
			multisig_id: T::AccountId,
			transaction_id: T::Hash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let multisig =
				Multisigs::<T>::get(&multisig_id).ok_or(Error::<T>::MultisigDoesNotExist)?;
			// Ensure the proposer is a member of the multisig
			ensure!(multisig.members.contains(&who), Error::<T>::NotAMember);
			let transaction = Transactions::<T>::get(&multisig_id, &transaction_id)
				.ok_or(Error::<T>::TransactionDoesNotExist)?;
			Self::deposit_event(Event::TransactionCanceled {
				submitter: who,
				transaction: transaction_id,
				multisig: multisig_id.clone(),
				status: TransactionStatus::Canceled,
				call_hash: transaction.call_hash,
			});
			Ok(())
		}
		/// WARNING: Only meant to be executed via propose transaction call dispatch.
		/// Dispatch function call to delete a multisig account and release all of "Hold" funds.
		/// The remaining funds including the hold will be sent to the creator of the account.
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::default())]
		pub fn delete_multisig(origin: OriginFor<T>, multisig_id: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let multisig =
				Multisigs::<T>::get(&multisig_id).ok_or(Error::<T>::MultisigDoesNotExist)?;
			// Ensure the proposer is a member of the multisig
			ensure!(multisig.members.contains(&who), Error::<T>::NotAMember);
			// Release all the "Hold" funds from the multisig account
			T::NativeBalance::release_all(
				&HoldReason::MultisigCreationDeposit.into(),
				&multisig_id,
				Precision::BestEffort,
			)?;
			// All funds in the multisig account to reap the account
			let total_funds = T::NativeBalance::reducible_balance(
				&multisig_id,
				Preservation::Expendable,
				Fortitude::Force,
			);
			// Transfer the remaining funds including the deposit to the creator of the multisig
			T::NativeBalance::transfer(
				&multisig_id,
				&multisig.creator,
				total_funds,
				Preservation::Expendable,
			)
			.map_err(|_| Error::<T>::TransferFailed)?;
			Multisigs::<T>::remove(&multisig_id);
			Self::deposit_event(Event::MultisigDeleted { from: who, multisig: multisig_id });
			Ok(())
		}
	}
}
