
> [!WARNING]
> The IOTA CLI currently does not support transaction sponsorship.
  Additionally, it would require significant effort to handle encoding and decoding of signatures, public keys, and transaction bytes. 
  So, the following steps outline the general flow to provide an overview of the process.
  For full-fledged flow, please use an [SDK example](../aa_rust_sdk_flow/)

1. Multisig Address Creation and Funding
1.1 Create a Multisignature Address
Create a multisig address comprising two participants (e.g., Alice and Bob) with a signature threshold of 2.
Refer to the official documentation:
[Multisig Guide](https://docs.iota.org/developer/cryptography/transaction-auth/multisig)
1.2 Fund the Multisig Address
Use the IOTA faucet to fund the generated multisig address:
    ```bash
    /Users/pk/iota client faucet --address $MULTISIG_ADDR
    ```

2. Deployment of the Account Abstraction (AA) Package via Multisig:
    2.1 Publish the Package
    Prepare the unsigned transaction for deployment:
    ```bash
    /Users/pk/iota client publish --gas $MULTISIG_ADDR_GAS --gas-budget 100000000 --serialize-unsigned-transaction 
    ```
    Export the serialized transaction: 
    ```bash
    export PUB_TX
    ```

    2.2 Sign the Transaction
    Sign the serialized transaction using both participants’ keys:

    ```bash
    /Users/pk/iota keytool sign --address $ALICE_ADDR --data $PUB_TX 
    /Users/pk/iota keytool sign --address $BOB_ADDR --data $PUB_TX 
    ``` 
    Export the individual signatures:
     ```bash
    export PUB_TX_SIGN1
    export PUB_TX_SIGN2
    ```

    2.3 Generate a Multisignature
    Combine both signatures into a single multisig signature:
    ```bash
    /Users/pk/iota keytool multi-sig-combine-partial-sig --pks $ALICE_PUBLIC_KEY $BOB_PUBLIC_KEY --weights 1 2 --threshold 2 --sigs $PUB_TX_SIGN1 $PUB_TX_SIGN2
    ```
    Export the combined multisignature:
    ```bash
    export PUB_TX_MULTISIG
    ```
    2.4 Execute the Deployment Transaction
    Execute the transaction using the multisignature:
    ```bash
    /Users/pk/iota client execute-signed-tx --tx-bytes $PUB_TX --signatures $PUB_TX_MULTISIG
    ```
    Export the package identifier:
    ```bash
    export PACKAGE_ID
    ```

3. Initialize the Smart Account
Invoke the smart account initialization function:
    ```bash
     /Users/pk/iota client call --package $PACKAGE_ID --module smart_account --function init_multisig_smart_account --args $MULTISIG_ADDR
    ```
    Export the resulting required values as:
    ```bash
    export AA
    export OWNER_CAP
    ```

4. Deposit Coins into the Smart Account
Deposit a coin object into the smart account’s balance:
    ```bash
     /Users/pk/iota client transfer --to $AA --object-id $SOME_COIN_OBJ
    ```

5. Receive Coins for the Smart Account
Receive a coin object into the smart account’s balance(**Sponsorship also required**):
    ```bash
      /Users/pk/iota client call --package $PACKAGE_ID --module smart_account --function receive_deposit --args $AA $SOME_COIN_OBJ 
    ```

6. Construct a Withdraw Transaction
Create a serialized unsigned transaction to withdraw tokens from the smart account (served with Alice’s gas object):
    ```bash
    /Users/pk/iota client ptb --move-call $PACKAGE_ID::smart_account::withdraw @$AA @$OWNER_CAP 9999999 --assign withdraw_coin \
    --move-call 0x2::transfer::public_transfer $SOME_COIN_OBJ @$RECIPIENT_ADDR
    ```
    Export the transaction bytes:
    ```bash
    export TX
    export TX_DIGEST
    ```

7. Submit the Transaction Proposal
Create a proposed transaction entry point using the serialized unsigned transaction:
    ```bash
    /Users/pk/iota client call --package $PACKAGE_ID --module tx_flow --function entry_point --args $AA $TX_DIGEST $TX 2
    ```
    Export the proposed transaction ID:
    ```bash
    export PROPOSED_TX_ID
    ```


> [!WARNING]
> The following steps involve manual processing of base64 signatures into raw components.

8. Sign the Transaction Proposal
Here actually we have to extract the pure signature(signature bytes with the prefix flag and the public_key suffix).
For instance we have a signature in base64 format:
```bash
AFhSeMS6tVuwJ9nzFDFLJLR3oEyKqmx1deaszpT5BU5IyLkUKDNDhUuoP329EDlwBhU7bEhXd+hX3M35n8CkFwqdUQKrysRzJaY8a9kivajuYiHD1lPTJOQtx9Tjxuv3Bg==
```
You need to decode it into hex:
```bash 00585278c4bab55bb027d9f314314b24b477a04c8aaa6c7575e6acce94f9054e48c8b914283343854ba83f7dbd10397006153b6c485777e857dccdf99fc0a4170a9d5102abcac47325a63c6bd922bda8ee6221c3d653d324e42dc7d4e3c6ebf706
```
Then you need to split this into 3 parts:
The first byte (2 chars) = 00 -> this is the flag
then 64 bytes (128 chars) - this is the REAL signature
```bash
585278c4bab55bb027d9f314314b24b477a04c8aaa6c7575e6acce94f9054e48c8b914283343854ba83f7dbd10397006153b6c485777e857dccdf99fc0a4170a
``` 
the last 32 bytes (64 chars)is the public key used to sign, a repetition of Alice's publicBase64Key (no flag) encoded in hex:
```bash
9d5102abcac47325a63c6bd922bda8ee6221c3d653d324e42dc7d4e3c6ebf706
```
Sign the transaction independently by both Alice and Bob:
    ```bash
    /Users/pk/iota keytool sign --address $ALICE_ADDR --data $TX
    /Users/pk/iota keytool sign --address $BOB_ADDR --data $TX
    ```
    Export their signatures:
    ```bash
    export PURE_SIGN1
    export PURE_SIGN2
    ```

9. Register the Signatures On-chain
Submit the signed transaction proposals:
    ```bash
    /Users/pk/iota client ptb --move-call $PACKAGE_ID::tx_flow::sign_proposed_tx @$AA @$PROPOSED_TX_ID '"$ALICE_PUBLIC_KEY"' '"$PURE_SIGN1"'
    /Users/pk/iota client ptb --move-call $PACKAGE_ID::tx_flow::sign_proposed_tx @$AA @$PROPOSED_TX_ID '"$BOB_PUBLIC_KEY"' '"$PURE_SIGN2"'
    ```
    Extract the fully signed transaction object and export it:
    ```bash
    export SIGNED_TX
    ```

10. Combine Verified Signatures
Here, we need to extract the 'pure' verified signatures and reconstruct the full signature by adding the prefix flag and public key suffix. Then, export them as $VERIFIED_SIGN1 and $VERIFIED_SIGN2.

Create a final multisignature:
    ```bash
    /Users/pk/iota keytool multi-sig-combine-partial-sig --pks $ALICE_PUBLIC_KEY $BOB_PUBLIC_KEY --weights 1 2 --threshold 2 --sigs $VERIFIED_SIGN1 $VERIFIED_SIGN2
    ```
    Export the multisignature:
    ```bash
    export TX_MULTISIG
    ```

11. Execute the Final Transaction(**Actually, this step doesn’t work in the CLI, because — as mentioned above — the CLI doesn’t support sponsorship logic. However, we need it here since ALICE is the sponsor and initiator of this transaction.**)
Expected call:
    ```bash
    /Users/pk/iota client execute-signed-tx --tx-bytes $SIGNED_TX --signatures $TX_MULTISIG $ALICE_SIG
    ```
Instead, you must use the IOTA SDK (Rust) to sponsor and execute this transaction.