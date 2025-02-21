## Overview

This document is focused on using Validator Tool.

**Caveat: this tool only supports Pending Validators and Active Validators at the moment.**

## Preparation

1. Make sure you have completed all the [prerequisites](https://docs.iota.org/developer/getting-started/install-iota#prerequisites).

2. Build the `iota` binary, which you will need for the genesis ceremony. This step can be done on any machine you like. It does not have to be done on the machine on which you will run the validator.

   1. Clone the git repo:

          git clone git@github.com:iotaledger/iota.git && cd iota

   2. Check out the commit we will be using for the testnet:

          git checkout testnet

   3. Build iota binary

          cargo build --bin iota

   4. Remember the path to your binary:

          export IOTA_BINARY="$(pwd)/target/debug/iota"

3. Run the following command to set up your IOTA account and CLI environment.

   1. If this is the first time running this program, it will ask you to provide an IOTA Fullnode Server URL and a meaningful environment alias. It will also generate a random key pair in `iota.keystore` and a config `client.yaml`. Swap in your validator account key if you already have one.

   2. If you already set it up, simply make sure
      a. `rpc` is correct in `client.yaml`.
      b. `active_address` is correct in `client.yaml`.
      b. `iota.keystore` contains your account key pair.

   If at this point you can't find where `client.yaml` or `iota.keystore` is or have other questions, read [IOTA Client CLI tutorial](https://docs.iota.org/references/cli/client).

```bash
$IOTA_BINARY client
```

4. To test you are connected to the network and configured your config correctly, run the following command to display your validator info.

```bash
$IOTA_BINARY validator display-metadata
```

## Using Validator Tool

#### Print Help Info

```bash
$IOTA_BINARY validator --help
```

#### Display Validator Metadata

```bash
$IOTA_BINARY validator display-metadata
```

or

```bash
$IOTA_BINARY validator display-metadata <validator-address>
```

to print another validator's information.

#### Update Validator Metadata

Run the following to see how to update validator metadata. Read description carefully about when the change will take effect.

```bash
$IOTA_BINARY validator update-metadata --help
```

You can update the following on-chain metadata:

1. name
2. description
3. image URL
4. project URL
5. network address
6. p2p address
7. primary address
8. authority public key
9. network public key
10. protocol public key

Notably, only the first 4 metadata listed above take effect immediately.

If you change any metadata from points 5 to 11, they will be changed only after the next epoch - **for these, you'll want to restart the validator program immediately after the next epoch, with the new key files and/or updated `validator.yaml` config. Particularly, make sure the new address is not behind a firewall.**

Run the following to see how to update each metadata.

```bash
$IOTA_BINARY validator update-metadata --help
```

#### Operation Cap

Operation Cap allows a validator to authorizer another account to perform certain actions on behalf of this validator. Read about [Operation Cap here](./validator-operation/validator-tasks#operation-cap).

The Operation Cap holder (either the validator itself or the delegatee) updates its Gas Price and reports validator peers with the Operation Cap.

#### Report Validators

To report validators peers, run

```bash
$IOTA_BINARY validator report-validator <reportee-address>
```

Add `--undo-report false` if it intents to undo an existing report.

Similarly, if the account is a delegatee, add `--operation-cap-id <operation-cap-id>` option to the command.

if the account itself is a validator and holds the Operation Cap. Or

```bash
$IOTA_BINARY validator update-gas-price --operation-cap-id <operation-cap-id> <gas-price>
```

if the account is a delegatee.

#### Become a Validator / Join Committee

To become a validator candidate, first run

```bash
$IOTA_BINARY validator make-validator-info <name> <description> <image-url> <project-url> <host-name> <gas_price>
```

This will generate a `validator.info` file and key pair files. The output of this command includes:

1. Four key pair files (Read [more here](./validator-operation/validator-tasks/#key-management)). ==Set their permissions with the minimal visibility (chmod 600, for example) and store them securely==. They are needed when running the validator node as covered below.
   a. If you follow this guide thoroughly, this key pair is actually copied from your `iota.keystore` file.
2. `validator.info` file that contains your validator info. **Double check all information is correct**.

Then run

```bash
$IOTA_BINARY validator become-candidate {path-to}validator.info
```

to submit an on-chain transaction to become a validator candidate. The parameter is the file path to the validator.info generated in the previous step. **Make sure the transaction succeeded (printed in the output).**

At this point you are validator candidate and can start to accept self staking and delegated staking.

**If you haven't, start a fullnode now to catch up with the network. When you officially join the committee but is not fully up-to-date, you cannot make meaningful contribution to the network and may be subject to peer reporting hence face the risk of reduced staking rewards for you and your delegators.**

Add stake to a validator's staking pool: https://docs.iota.org/references/framework/iota-system/iota_system#function-request_add_stake

Once you collect enough staking amount, run

```bash
$IOTA_BINARY validator join-committee
```

to become a pending validator. A pending validator will become active and join the committee starting from next epoch.

#### Leave Committee

To leave committee, run

```bash
$IOTA_BINARY validator leave-committee
```

Then you will be removed from committee starting from next epoch.

### Generate the payload to create PoP

Serialize the payload that is used to generate Proof of Possession. This is allows the signer to take the payload offline for an Authority protocol BLS keypair to sign.

```bash
$IOTA_BINARY validator serialize-payload-pop --account-address $ACCOUNT_ADDRESS --protocol-public-key $BLS_PUBKEY
Serialized payload: $PAYLOAD_TO_SIGN
```

## Test becoming a validator in a local network

#### Start a local network with larger faucet amount

Set a larger faucet amount, so a single faucet request provides enough coins to become a validator.

```bash
iota start --force-regenesis --with-faucet --faucet-amount 2600000000000000
```

#### Request coins from the faucet

```bash
iota client switch --env localnet
```

Then request funds:

```bash
iota client faucet --url http://127.0.0.1:9123/gas
```

#### Make validator info

```bash
iota validator make-validator-info validator0 description https://iota.org/logo.png https://www.iota.org 127.0.0.1 1000
```

#### Become a validator

```bash
iota validator become-candidate validator.info
```

#### Stake funds

Get an object id for a gas coin to stake and the address to stake to:

```bash
iota client gas && iota client active-address
```

Stake the coin object:

```bash
iota client call --package 0x3 --module iota_system --function request_add_stake --args 0x5 <gas-coin-id> <validator-address> --gas-budget 10000000
```

Example where 0xfff7... is a coin object id, 0x111... is the validator address:

```bash
iota client call --package 0x3 --module iota_system --function request_add_stake --args 0x5 0xfff7d5a924a599e811e307c3eeb65d69906054466ac098a2715a19ab802ddf15 0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215 --gas-budget 10000000
```

All in one:

```bash
coinObjectId=$(iota client gas --json | jq '.[0].gasCoinId')
validatorAddress=$(iota client active-address)
iota client call --package 0x3 --module iota_system --function request_add_stake --args 0x5 $coinObjectId $validatorAddress --gas-budget 10000000
```

#### Finally, join the committee

```bash
iota validator join-committee
```

#### Combined

First terminal:

```bash
iota start --force-regenesis --with-faucet --faucet-amount 2600000000000000
```

Second terminal after the faucet is up:

```bash
iota client switch --env localnet
iota client faucet --url http://127.0.0.1:9123/gas
sleep 2 
iota validator make-validator-info validator0 description https://iota.org/logo.png https://www.iota.org 127.0.0.1 1000
iota validator become-candidate validator.info
sleep 2
coinObjectId=$(iota client gas --json | jq '.[0].gasCoinId')
validatorAddress=$(iota client active-address)
iota client call --package 0x3 --module iota_system --function request_add_stake --args 0x5 $coinObjectId $validatorAddress --gas-budget 10000000
sleep 2
iota validator join-committee
sleep 2
iota validator display-metadata
```
