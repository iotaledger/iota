# Account Abstraction Rust SDK Flow (IOTA)

This project demonstrates the Account Abstraction (AA) lifecycle - specifically Smart Account creation, withdrawal, and deletion - on the IOTA network using the Rust SDK and programmable Move smart contracts.

### It showcases how to:
1. Setup multi-signature accounts.
2. Deploy a Smart Account on-chain.
3. Deposit funds into the smart account.
4. Construct and propose a programmable transaction (withdrawal).
5. Collect multisignatures and proposed transaction on-chain.
6. Execute a multisigned transaction.
7. Delete the smart account.

## Modules Overview

 - main.rs - entry point executing the full Smart Account lifecycle.
 - tx_flow.rs - logic for proposing a transaction and collecting on-chain signatures.
 - smart_account.rs - logic for publishing the Account Abstraction Move package, initializing and deleting the SmartAccount.
 - signed_tx.rs - helper to deserialize the final SignedTx object from chain response.
 - sig_utils.rs - signature manipulation utilities (reconstruct, extract, and create multisig).
 - utils.rs - shared constants and helpers (e.g., gas, threshold, mnemonic, Move package compilation).
 - faucet.rs - helper for initial accounts funding.


## Execution Flow

1. Initialize IOTA Client and Keystore
    - Connect to a localnet.
    - Create in-memory key store and derive Alice and Bob accounts from mnemonic.

2. Build Multisig Address
    Use both public keys with custom WEIGHTS and THRESHOLD to derive a shared multisig address.

3. Request Tokens via Faucet
    Request initial tokens for Alice, Bob, and the multisig address.

4. Publish AA Move Package
    - Compile and deploy Move smart contracts from `../aa_move` using the multisig address as payer.
    - Store returned package_id for later usage.

5. Initialize SmartAccount
    - Call `init_multisig_smart_account` from the deployed Move package.
    Creates and returns two Move objects:
    - SmartAccount - shared object.
    - OwnerCap - capability object used for privileged calls like withdraw or delete.

6. Deposit Funds to SmartAccount
    - Transfer a coin from Alice to the SmartAccount.
    - Then call `receive_deposit()` to complete the ownership transfer.

7. Prepare a Withdraw Transaction
    - Build a programmable withdrawal transaction to send tokens from the SmartAccount to an external recipient.
    - Package it as `TransactionData` and derive a `transaction digest`.

8. Propose Transaction On-Chain
    Submit a ProposedTx Move object to chain including:
    - digest
    - raw transaction bytes
    - threshold

9. Sign Proposed Transaction
    - Each signer (Alice and Bob) registers their signature on-chain by calling `sign_proposed_tx()`.
    - After reaching the threshold, a SignedTx object is created automatically.

10. Extract and Execute the SignedTx
    Download the SignedTx object, decode it to reconstruct:
    - Original `TransactionData`
    - Multisignature (combined from individual `GenericSignatures`)
    - Execute the final transaction using `execute_transaction_block()` with the multisig + proposer's signature.

11. Check Result
    - Verify recipient address received the expected amount.

12. Delete SmartAccount
    - Initiated by Bob, co-signed by Alice.
    Calls `delete_multisig_smart_account()` to:
    - delete SmartAccount and OwnerCap
    - reclaim coins to Bob's account