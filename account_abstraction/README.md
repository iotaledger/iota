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
     /Users/pk/iota client call --package $PACKAGE_ID --module account_abstraction --function init_multisig_aa --args $MULTISIG_ADDR
    ```
    Export the resulting required values as:
    ```bash
    export AA
    export OWNER_CAP
    ```

4. Deposit Coins into the Smart Account
Deposit a coin object into the smart account’s balance:
    ```bash
    /Users/pk/iota client call --package $PACKAGE_ID --module account_abstraction --function deposit --args $AA $SOME_COIN_OBJ
    ```

5. Construct a Withdraw Transaction
Create a serialized unsigned transaction to withdraw tokens from the smart account (served with Alice’s gas object):
    ```bash
    /Users/pk/iota client call --package $PACKAGE_ID --module account_abstraction --function withdraw --args $AA $OWNER_CAP 9999999 $RECIPIENT_ADDR --gas $ALICE_GAS_OBJ --gas-budget 1000000 --serialize-unsigned-transaction
    ```
    Export the transaction bytes:
    ```bash
    export TX
    ```

6. Submit the Transaction Proposal
Create a proposed transaction entry point using the serialized unsigned transaction:
    ```bash
    /Users/pk/iota client call --package $PACKAGE_ID --module tx_flow --function entry_point --args $AA $TX 2
    ```
    Export the proposed transaction ID:
    ```bash
    export PROPOSED_TX_ID
    ```

7. Sign the Transaction Proposal
Sign the transaction independently by both Alice and Bob:
    ```bash
    /Users/pk/iota keytool sign --address $ALICE_ADDR --data $TX
    /Users/pk/iota keytool sign --address $BOB_ADDR --data $TX
    ```
    Export their signatures:
    ```bash
    export SIGN1
    export SIGN2
    ```

8. Register the Signatures On-chain
Submit the signed transaction proposals:
    ```bash
    /Users/pk/iota client ptb --move-call $PACKAGE_ID::tx_flow::sign_proposed_tx @$AA @$PROPOSED_TX_ID '"$ALICE_PUBLIC_KEY"' '"$SIGN1"'
    /Users/pk/iota client ptb --move-call $PACKAGE_ID::tx_flow::sign_proposed_tx @$AA @$PROPOSED_TX_ID '"$BOB_PUBLIC_KEY"' '"$SIGN2"'
    ```
    Extract the fully signed transaction object and export it:
    ```bash
    export SIGNED_TX
    ```

9. Combine Verified Signatures
Extract verified individual signatures and create a final multisignature:
    ```bash
    /Users/pk/iota keytool multi-sig-combine-partial-sig --pks $ALICE_PUBLIC_KEY $BOB_PUBLIC_KEY --weights 1 2 --threshold 2 --sigs $VERIFIED_SIGN1 $VERIFIED_SIGN2
    ```
    Export the multisignature:
    ```bash
    export TX_MULTISIG
    ```

10. Execute the Final Transaction
Execute the multisigned transaction on-chain:
    ```bash
    /Users/pk/iota client execute-signed-tx --tx-bytes $SIGNED_TX --signatures $TX_MULTISIG
    ```