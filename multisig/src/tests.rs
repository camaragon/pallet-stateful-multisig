use crate::{mock::*, *};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate, BoundedBTreeMap};
use sp_core::blake2_256;

#[test]
fn generate_multi_account_id_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let nonce = MultisigNonce::<Test>::get();
		let multisig_account_id = Multisig::generate_multi_account_id(nonce);
		let regenerated = Multisig::generate_multi_account_id(nonce);
		// Check that the generated account ID is deterministic
		assert_eq!(multisig_account_id, regenerated);
	});
}

#[test]
fn generate_transaction_id_works() {
	new_test_ext().execute_with(|| {
		let to = 3;
		let amount: u128 = 1000u128.into();
		let call = call_transfer(to, amount);
		let call_hash = blake2_256(&call.encode());
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let proposer = 1;
		let transaction_id =
			Multisig::generate_transaction_id(proposer, System::block_number(), call_hash);
		let regenerated =
			Multisig::generate_transaction_id(proposer, System::block_number(), call_hash);
		// Check that the generated account ID is deterministic
		assert_eq!(transaction_id, regenerated);
	});
}

#[test]
fn tally_vote_counts_per_status() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let status = TransactionStatus::Pending;
		let mut votes = BoundedBTreeMap::<
			<Test as frame_system::Config>::AccountId,
			Vote,
			<Test as Config>::MaxMembers,
		>::new();
		votes.try_insert(1, Vote::Approve).unwrap();
		votes.try_insert(2, Vote::Reject).unwrap();
		votes.try_insert(3, Vote::Approve).unwrap();
		let (approvals, rejections) = Multisig::do_tally_votes(status, votes).unwrap();
		assert_eq!(approvals, 2);
		assert_eq!(rejections, 1);
	});
}

#[test]
fn build_transaction_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let from = 1;
		let multisig_id = 2;
		let to = 3;
		let amount: u128 = 1000u128.into();
		let call = call_transfer(to, amount);
		let call_hash = blake2_256(&call.encode());
		assert_ok!(Multisig::build_transaction(from, multisig_id, call.clone(), call_hash));
		let transaction_id =
			Multisig::generate_transaction_id(from, System::block_number(), call_hash);
		let new_transaction = Transactions::<Test>::get(&multisig_id, &transaction_id)
			.expect("Transaction should exist");
		assert_eq!(new_transaction.proposer, from);
		assert_eq!(new_transaction.status, TransactionStatus::Pending);
		assert_eq!(new_transaction.call, call);
		assert_eq!(new_transaction.call_hash, call_hash);
		assert_eq!(new_transaction.votes.len(), 1);
		assert_eq!(new_transaction.votes.get(&from), Some(&Vote::Approve));
		assert_eq!(new_transaction.created_at, System::block_number());
		assert_eq!(
			new_transaction.expires_at,
			System::block_number().saturating_add(DEFAULT_EXPIRATION_BLOCKS)
		);
		System::assert_last_event(
			Event::TransactionCreated {
				proposer: from,
				transaction: transaction_id,
				multisig: multisig_id,
				status: TransactionStatus::Pending,
				call_hash,
			}
			.into(),
		);
	});
}

#[test]
fn create_new_multisig_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		Balances::set_balance(&creator, 1_000u128.into());
		let members = generate_members();
		let nonce = MultisigNonce::<Test>::get();
		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));
		let multisig_id = Multisig::generate_multi_account_id(nonce);
		let new_multisig = Multisigs::<Test>::get(&multisig_id).expect("Multisig should exist");
		assert_eq!(new_multisig.creator, creator);
		assert_eq!(new_multisig.members, members);
		assert_eq!(new_multisig.threshold, 2);
		assert_eq!(new_multisig.created_at, System::block_number());
		System::assert_last_event(Event::NewMultisig { creator, multisig: multisig_id }.into());
	});
}

#[test]
fn fund_multisig_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		Balances::set_balance(&creator, 1_000_000u128.into());
		let members = generate_members();
		let amount: u128 = 1_000u128.into();
		let nonce = MultisigNonce::<Test>::get();
		let multisig_id = Multisig::generate_multi_account_id(nonce);

		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));

		assert_ok!(Multisig::fund_multisig(RuntimeOrigin::signed(creator), multisig_id, amount));

		let total_balance = amount.saturating_add(1u32.into());
		let multisig_balance = Balances::free_balance(&multisig_id);
		assert_eq!(multisig_balance, total_balance);
		System::assert_last_event(
			Event::MultisigFunded { from: creator, to: multisig_id, amount }.into(),
		);
	});
}

#[test]
fn propose_transaction_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		Balances::set_balance(&creator, 1_000_000u128.into());
		let to = 2;
		let members = generate_members();
		let amount: u128 = 1_000u128.into();
		let nonce = MultisigNonce::<Test>::get();
		let call = call_transfer(to, amount);
		let call_hash = blake2_256(&call.encode());
		let multisig_id = Multisig::generate_multi_account_id(nonce);
		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));
		assert_ok!(Multisig::propose_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			call,
		));
		let transaction_id =
			Multisig::generate_transaction_id(creator, System::block_number(), call_hash);
		let new_transaction = Transactions::<Test>::get(&multisig_id, &transaction_id)
			.expect("Transaction should exist");
		assert_eq!(new_transaction.proposer, creator);
		assert_eq!(new_transaction.status, TransactionStatus::Pending);
	});
}

#[test]
fn vote_on_transaction_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		Balances::set_balance(&creator, 1_000_000u128.into());
		let to = 3;
		let members = generate_members();
		let amount: u128 = 1_000u128.into();
		let nonce = MultisigNonce::<Test>::get();
		let vote: Vote = Vote::Approve;
		let call = call_transfer(to, amount);
		let call_hash = blake2_256(&call.encode());
		let multisig_id = Multisig::generate_multi_account_id(nonce);
		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));
		assert_ok!(Multisig::propose_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			call,
		));
		let transaction_id =
			Multisig::generate_transaction_id(creator, System::block_number(), call_hash);
		assert_ok!(Multisig::vote(RuntimeOrigin::signed(2), multisig_id, transaction_id, vote));
		let new_transaction = Transactions::<Test>::get(&multisig_id, &transaction_id)
			.expect("Transaction should exist");
		assert_eq!(new_transaction.votes.len(), 2);
	});
}

#[test]
fn submit_proposed_transaction_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		// Set the balance of the creator to ensure they can fund the transaction
		Balances::set_balance(&creator, 1_000_000u128.into());
		let to = 3;
		let members = generate_members();
		let amount: u128 = 1_000u128.into();
		let nonce = MultisigNonce::<Test>::get();
		let call = call_transfer(to, amount);
		let call_hash = blake2_256(&call.encode());
		let multisig_id = Multisig::generate_multi_account_id(nonce);
		// Set the balance of the multisig account to ensure it can fund the transaction
		Balances::set_balance(&multisig_id, 1_000_000u128.into());
		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(1)
		));
		assert_ok!(Multisig::propose_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			call.clone(),
		));
		let transaction_id =
			Multisig::generate_transaction_id(creator, System::block_number(), call_hash);
		assert_ok!(Multisig::submit_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			transaction_id,
			call,
			call_hash
		));
		assert!(
			Transactions::<Test>::get(&multisig_id, &transaction_id).is_none(),
			"Transaction should be removed after submission"
		);
		System::assert_last_event(
			Event::TransactionExecuted {
				submitter: creator,
				transaction: transaction_id,
				multisig: multisig_id,
				approvals: 1,
				rejections: 0,
				status: TransactionStatus::Complete,
				call_hash,
			}
			.into(),
		);
	});
}

#[test]
fn cancel_proposed_transaction() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		// Set the balance of the creator to ensure they can fund the transaction
		Balances::set_balance(&creator, 1_000_000u128.into());
		let to = 3;
		let members = generate_members();
		let amount: u128 = 1_000u128.into();
		let nonce = MultisigNonce::<Test>::get();
		let multisig_id = Multisig::generate_multi_account_id(nonce);
		let proposed_call = call_transfer(to, amount);
		let proposed_call_hash = blake2_256(&proposed_call.encode());
		let proposed_transaction_id =
			Multisig::generate_transaction_id(creator, System::block_number(), proposed_call_hash);
		let call = call_cancel_transaction(multisig_id, proposed_transaction_id);
		let call_hash = blake2_256(&call.encode());
		let transaction_id =
			Multisig::generate_transaction_id(creator, System::block_number(), call_hash);
		// Set the balance of the multisig account to ensure it can fund the transaction
		Balances::set_balance(&multisig_id, 1_000_000u128.into());
		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));
		// Build and propose a transaction
		assert_ok!(Multisig::build_transaction(
			creator,
			multisig_id,
			proposed_call.clone(),
			proposed_call_hash
		));
		assert_ok!(Multisig::propose_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			proposed_call.clone(),
		));
		// Build and propose the cancelation transaction of an existing transaction
		assert_ok!(Multisig::build_transaction(creator, multisig_id, call.clone(), call_hash));
		assert_ok!(Multisig::propose_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			call.clone(),
		));
		assert_ok!(Multisig::vote(
			RuntimeOrigin::signed(2),
			multisig_id,
			transaction_id,
			Vote::Approve
		));
		assert_ok!(Multisig::vote(
			RuntimeOrigin::signed(3),
			multisig_id,
			transaction_id,
			Vote::Approve
		));
		assert_ok!(Multisig::submit_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			transaction_id,
			call,
			call_hash
		));
		assert!(
			Transactions::<Test>::get(&multisig_id, &transaction_id).is_none(),
			"Transaction should be removed after cancellation"
		);
		System::assert_has_event(
			Event::TransactionCanceled {
				submitter: creator,
				transaction: proposed_transaction_id,
				multisig: multisig_id,
				status: TransactionStatus::Canceled,
				call_hash: proposed_call_hash,
			}
			.into(),
		);
	});
}

#[test]
fn delete_multisig_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		// Set the balance of the creator to ensure they can fund the transaction
		Balances::set_balance(&creator, 1_000_000u128.into());
		let members = generate_members();
		let nonce = MultisigNonce::<Test>::get();
		let multisig_id = Multisig::generate_multi_account_id(nonce);
		// Set the balance of the multisig account to ensure it can fund the transaction
		Balances::set_balance(&multisig_id, 1_000_000u128.into());
		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));
		let call = call_delete_multisig(multisig_id);
		let call_hash = blake2_256(&call.encode());
		assert_ok!(Multisig::propose_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			call.clone(),
		));
		let transaction_id =
			Multisig::generate_transaction_id(creator, System::block_number(), call_hash);
		assert_ok!(Multisig::vote(
			RuntimeOrigin::signed(2),
			multisig_id,
			transaction_id,
			Vote::Approve
		));
		assert_ok!(Multisig::submit_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			transaction_id,
			call,
			call_hash
		));
		System::assert_has_event(
			Event::MultisigDeleted { from: creator, multisig: multisig_id }.into(),
		);
		System::assert_last_event(
			Event::TransactionExecuted {
				submitter: creator,
				transaction: transaction_id,
				multisig: multisig_id,
				approvals: 2,
				rejections: 0,
				status: TransactionStatus::Complete,
				call_hash,
			}
			.into(),
		);
	});
}

#[test]
fn fund_multisig_does_not_exist() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		Balances::set_balance(&creator, 1_000_000u128.into());
		let multisig_id = 2;
		let amount: u128 = 1_000u128.into();

		assert_noop!(
			Multisig::fund_multisig(RuntimeOrigin::signed(creator), multisig_id, amount),
			Error::<Test>::MultisigDoesNotExist
		);
	});
}

#[test]
fn can_only_vote_once() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		Balances::set_balance(&creator, 1_000_000u128.into());
		let to = 2;
		let members = generate_members();
		let amount: u128 = 1_000u128.into();
		let nonce = MultisigNonce::<Test>::get();
		let vote: Vote = Vote::Approve;
		let call = call_transfer(to, amount);
		let call_hash = blake2_256(&call.encode());
		let multisig_id = Multisig::generate_multi_account_id(nonce);
		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));
		assert_ok!(Multisig::propose_transaction(
			RuntimeOrigin::signed(creator),
			multisig_id,
			call,
		));
		let transaction_id =
			Multisig::generate_transaction_id(creator, System::block_number(), call_hash);
		assert_noop!(
			Multisig::vote(RuntimeOrigin::signed(creator), multisig_id, transaction_id, vote),
			Error::<Test>::AlreadyVoted
		);
	});
}

#[test]
fn multisig_creator_must_be_member() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		// Dispatch a signed extrinsic.
		let creator = 5;
		let members = generate_members();

		assert_noop!(
			Multisig::create_multisig(RuntimeOrigin::signed(creator), members.clone(), None),
			Error::<Test>::ProposerMustBeMember
		);
	});
}

#[test]
fn multisig_threshold_too_low() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		let members = generate_members();

		assert_noop!(
			Multisig::create_multisig(RuntimeOrigin::signed(creator), members.clone(), Some(5)),
			Error::<Test>::ThresholdTooHigh
		);
	});
}

#[test]
fn multisig_creator_not_enough_funds() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		let members = generate_members();

		assert_noop!(
			Multisig::create_multisig(RuntimeOrigin::signed(creator), members.clone(), Some(2)),
			Error::<Test>::NotEnoughFunds
		);
	});
}

#[test]
fn fund_multisig_zero_amount() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;

		assert_noop!(
			Multisig::fund_multisig(RuntimeOrigin::signed(creator), 2, 0),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn fund_multisig_not_enough_funds() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;

		assert_noop!(
			Multisig::fund_multisig(RuntimeOrigin::signed(creator), 2, 100),
			Error::<Test>::NotEnoughFunds
		);
	});
}

#[test]
fn propose_transaction_multisig_non_existent() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let to = 2;
		let amount: u128 = 1_000u128.into();
		let nonce = MultisigNonce::<Test>::get();
		let call = call_transfer(to, amount);
		let multisig_id = Multisig::generate_multi_account_id(nonce);

		assert_noop!(
			Multisig::propose_transaction(RuntimeOrigin::signed(5), multisig_id, call),
			Error::<Test>::MultisigDoesNotExist
		);
	});
}

#[test]
fn propose_transaction_non_member() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let creator = 1;
		// Set the balance of the creator to ensure they can fund the transaction
		Balances::set_balance(&creator, 1_000_000u128.into());
		let amount: u128 = 1_000u128.into();
		let members = generate_members();
		let nonce = MultisigNonce::<Test>::get();
		let call = call_transfer(10, amount);
		let multisig_id = Multisig::generate_multi_account_id(nonce);

		assert_ok!(Multisig::create_multisig(
			RuntimeOrigin::signed(creator),
			members.clone(),
			Some(2)
		));
		assert_noop!(
			Multisig::propose_transaction(RuntimeOrigin::signed(10), multisig_id, call),
			Error::<Test>::ProposerMustBeMember
		);
	});
}
