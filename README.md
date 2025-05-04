<h1 align="center">Multisig Substrate Pallet</h1>

## Overview

This project implements a **stateful multi-signature pallet** for the Substrate runtime that enables multiple members to manage a shared account holding on-chain funds. Members can propose dispatchable calls on behalf of the multisig account, each tied to a unique transaction.

## Features
- Shared Account Control — Multisig accounts owned by a group of members.
- Proposal System — Transactions are proposed and tied to a dispatched call then executed once approved.
- Voting Mechanism — Members vote to approve or reject a proposed transaction.
- Cancellation Support — Proposed transactions can be canceled before execution.
- Multisig Deletion — Multisig accounts can be deleted when no longer used or needed.

## Instructions

An `omni-node` is broadly a
substrate-based node that has no dependency to any given runtime, and can run a wide spectrum of
runtimes. The `omni-node` provided below is based on the aforementioned template and therefore has
no consensus engine baked into it.

### Pallet

```rust
impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type NativeBalance = Balances;
	type RuntimeCall = RuntimeCall;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MaxMembers = ConstU32<10>;
	type DefaultThreshold = ConstU32<6>;
	type DefaultExpirationBlocks = ConstU32<100>;
	type MultisigDeposit = ConstU128<10>;
}

parameter_types! {
	pub const AssetDeposit: Balance = 100;
	pub const ApprovalDeposit: Balance = 1;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: Balance = 10;
	pub const MetadataDepositPerByte: Balance = 1;
}
```

To test while developing, without a full build:

```sh
cargo t -p pallet-multisig
```

### Entire Runtime

#### Using `omni-node`

First, make sure to install the special omni-node of the PBA assignment, if you have not done so
already from the previous activity.

```sh
cargo install polkadot-omni-node --locked
cargo install staging-chain-spec-builder --locked
```

Then build your runtime Wasm:

```sh
cargo build -p pba-runtime --release
```

Then create a chain-spec from that Wasm file:

```sh
chain-spec-builder create --runtime ./target/release/wbuild/pba-runtime/pba_runtime.wasm --relay-chain westend --para-id 1000 -t development default
```

Then load that chain-spec into the omninode:

```sh
polkadot-omni-node --chain chain_spec.json --dev-block-time 6000 --tmp
```

Populate your chain-spec file then with more accounts, like:

```json
// Find the existing, but empty `balances` key in the existing JSON, and update that.
"balances": {
  "balances": [
    ["5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY", 100000000000],
    ["5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty", 100000000000],
    ["5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y", 100000000000],
    ["5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy", 100000000000],
    ["5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw", 100000000000],
    ["5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL", 100000000000],
    ["5GNJqTPyNqANBkUVMN1LPPrxXnFouWXoe2wNSmmEoLctxiZY", 100000000000],
    ["5HpG9w8EBLe5XCrbczpwq5TSXvedjrBGCwqxK1iQ7qUsSWFc", 100000000000],
    ["5Ck5SLSHYac6WFt5UZRSsdJjwmpSZq85fd5TRNAdZQVzEAPT", 100000000000],
    ["5HKPmK9GYtE1PSLsS1qiYU9xQ9Si1NcEhdeCq9sw5bqu4ns8", 100000000000],
    ["5FCfAonRZgTFrTd9HREEyeJjDpT397KMzizE6T3DvebLFE7n", 100000000000],
    ["5CRmqmsiNFExV6VbdmPJViVxrWmkaXXvBrSX8oqBT8R9vmWk", 100000000000],
    ["5Fxune7f71ZbpP2FoY3mhYcmM596Erhv1gRue4nsPwkxMR4n", 100000000000],
    ["5CUjxa4wVKMj3FqKdqAUf7zcEMr4MYAjXeWmUf44B41neLmJ", 100000000000]
  ]
}
```

And more details like:

```json
"chainType": "Development"
"properties": {
  "tokenDecimals": 1,
  "tokenSymbol": "PBA"
}
```
## Implementation

The multisig pallet was implemented with the intention of being safe and the usage of minimal storage:
- `Multisigs` - The multisigs are stored using `StorageMap` hashed to with a prefix to be more unbalanced in the trie for easier lookup.
- `Transactions` - The transactions are stored using a `StorageDoubleMap` with a prefix as well for an unbalanced lookup in the trie. The first is the hashed key of the Multisig the transactions belong to. The second key is the hash of the transaction themselves.
- `MultisigNonce` - A `StorageValue` of the nonce for every new multisig created.

some configurable constants were also provided:
- `MultisigDeposit` - Deposit to be taken on creation of a multisig account by the creator. To be returned to the creator on multisig account deletion.
- `MaxMembers` - Max limit of members allowed to join the multisig.
- `DefaultThreshold` - Default threshold set for a proposed transaction to be executed or rejected.
- `DefaultExpirationBlocks` - Default blocks to be added to the created block to find the expiry block.

Here are all the dispatch extrinsic calls:
- `create_multisig`
- `fund_multisig`
- `propose_transaction`
- `vote`
- `submit_transaction`
- `cancel_transaction`
- `delete_multisig`

I relied on enums to provide different states/statuses:
- `Vote`
- `TransactionStatus`

The multisig id is generated using the nonce so every multisig account id will be different. A configurable deposit is required to create the multisig which helps prevent users from spamming creation of them. There are several safety checks to ensure that the creator of the multisig is also wanting to be a member. 

A fund dispatch was created that bypasses the proposal process and allows non members to fund the multisig. A member has the ability to propose a transaction where a call can be stored and is hashed for verifability during submission. The proposed transaction is then voted on with the option of "Approve" or "Reject" through the `Vote` enum. Once the transaction has reached its threshold for approvals the hash of the dispatch call is verified and executed. For opposite the transaction is canceled.

 All transactions are deleted from storage despite whether executed or canceled. A user can also cancel a transaction during it's proposal process and prior to a threshold being met. Although, that cancel transaction must be proposed and voted upon before executing. In the case that a multisig is no longer necesary or used there is the ability to delete the multisig, but it must go through the proposal process in order to execute. All of this is implemented with many safety checks in place ensuring a multisig account and its member's funds are safe.

## Learning Highlights
- First time working with such an advanced level of Rust including the generic types and macro usage.
	- I feel as if my rust and programming skills as a whole leveled up from this project. I loved the modularity and reusability that the generic types brought to the development process. I'm excited for my next Rust project as I will be much better because of this.
- Working in the runtime of a blockchain brings on a whole new level of thoughtful software development.
- I have a new understanding of how the blockchain fully works after building a runtime on it.
- The list can really go and on...

## Future Improvements
While I feel like I accomplished a lot in the short time given for this project here are some improvements I have in mind:
- Add more sad path tests for better test coverage.
- Simulate “attack vectors” by chaining extrinsics and testing state transitions under edge conditions or race scenarios with better integration tests.
- Replace the current +1 workaround for taking a deposit with a more accurate and scalable method to account for transfer fees when reserving the multisig deposit.
- Introduce more helper functions in the test mocks to streamline repetitive setup logic and improve test readability.
- Break larger dispatch functions into smaller, implementaion functions to improve maintainability and reduce cognitive complexity.
- Take a deposit for proposed transactions to prevent spamming.
- Setup benchmarking to accurately calculate the weights of each call.
- A migration plan to a stateless design similar to what the polkadot sdk multisig implements. 

Created by Cameron Aragon - PBA Assignment
