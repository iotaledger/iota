/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating the claim of a CoinManagerTreasuryCap related to a
 * foundry output. In order to work, it requires a network with test objects
 * generated from iota-genesis-builder/src/stardust/test_outputs.
 */
import {getFullnodeUrl, IotaClient, IotaObjectData} from "@iota/iota-sdk/client";
import {Ed25519Keypair} from "@iota/iota-sdk/keypairs/ed25519";
import {fundAddress} from "../utils";
import {bcs} from "@iota/iota-sdk/bcs";
import * as assert from "node:assert";
import {Transaction} from "@iota/iota-sdk/transactions";

const MAIN_ADDRESS_MNEMONIC = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";
const STARDUST_PACKAGE_ID = "0x107a";
const COIN_MANAGER_MODULE_NAME = "coin_manager";
const COIN_MANAGER_TREASURY_CAP_STRUCT_NAME = "CoinManagerTreasuryCap";
const IOTA_FRAMEWORK_ADDRESS = "0x2";

async function main() {
    // Build a client to connect to the local IOTA network.
    const iotaClient = new IotaClient({url: getFullnodeUrl('localnet')});

    // Derive the address of the first account.
    const keypair = Ed25519Keypair.deriveKeypair(MAIN_ADDRESS_MNEMONIC);
    const sender = keypair.toIotaAddress();
    console.log(`Sender address: ${sender}`);

    // Fund the sender address.
    await fundAddress(iotaClient, sender);

    // This object id was fetched manually. It refers to an Alias Output object that
    // contains a CoinManagerTreasuryCap (i.e., a Foundry representation).
    const aliasOutputObjectId = "0xa58e9b6b85863e2fa50710c4594f701b2f5e2c6ff5e3c2b10cf09e6b18d740da";
    const aliasOutputObject = await iotaClient.getObject({id: aliasOutputObjectId});
    if (!aliasOutputObject) {
        throw new Error("Alias output object not found");
    }

    // Get the dynamic field owned by the Alias Output, i.e., only the Alias
    // object.
    // The dynamic field name for the Alias object is "alias", of type vector<u8>.
    const dfName = {
        type: bcs.TypeTag.serialize({
            vector: {
                u8: true,
            },
        }).parse(),
        value: "alias"
    };

    const aliasObject = await iotaClient.getDynamicFieldObject({ parentId: aliasOutputObjectId, name: dfName});
    if (!aliasObject) {
        throw new Error("Alias object not found");
    }

    // Get the object id of the Alias object.
    const aliasObjectId = aliasObject.data?.objectId;

    // Get the objects owned by the alias object and filter in the ones with
    // CoinManagerTreasuryCap as type.
    const aliasOwnedObjects = await iotaClient.getOwnedObjects({
        owner: aliasObjectId ? aliasObjectId.toString() : "",
        options: {
            showBcs: true,
            showType: true
        },
    });

    // Only one page should exist.
    assert.ok(!aliasOwnedObjects.hasNextPage, "Only one page should exist");

    // Get the CoinManagerTreasuryCaps from the query.
    const ownedCoinManagerTreasuryCaps = aliasOwnedObjects.data
        .filter(object => {
            return isCoinManagerTreasuryCap(object.data as IotaObjectData);
        });

    // Get only the first coin manager treasury cap.
    const coinManagerTreasuryCap = ownedCoinManagerTreasuryCaps[0]?.data;
    if (!coinManagerTreasuryCap) {
        throw new Error("CoinManagerTreasuryCap not found");
    }

    // Extract the foundry token type from the type parameters of the coin manager
    // treasury cap object.
    const coinManagerTreasuryCapId = coinManagerTreasuryCap.objectId;
    const foundryTokenTypeStructTag = coinManagerTreasuryCap.type
    const foundryTokenType = foundryTokenTypeStructTag?.split("<")[1].split(">")[0] || "";

    // Create a PTB to claim the CoinManagerTreasuryCap related to the foundry
    // output from the alias output.
    const tx = new Transaction();
    // Type argument for an AliasOutput coming from the IOTA network, i.e., the
    // IOTA token or the Gas type tag.
    const gasTypeTag = "0x2::iota::IOTA";
    // Then pass the AliasOutput object as an input.
    const args = [tx.object(aliasOutputObjectId)];
    // Finally call the alias_output::extract_assets function.
    const extractedAliasOutputAssets = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::alias_output::extract_assets`,
        typeArguments: [gasTypeTag],
        arguments: args,
    });

    // The alias output can always be unlocked by the governor address. So the
    // command will be successful and will return a `base_token` (i.e., IOTA)
    // balance, a `Bag` of the related native tokens and the related Alias object.
    const extractedBaseToken = extractedAliasOutputAssets[0];
    const extractedNativeTokensBag = extractedAliasOutputAssets[1];
    const alias = extractedAliasOutputAssets[2];

    // Extract the IOTA balance.
    const iotaCoin = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: [gasTypeTag],
        arguments: [extractedBaseToken],
    });

    // Transfer the IOTA balance to the sender.
    tx.transferObjects([iotaCoin], tx.pure.address(sender));

    // In this example the native tokens bag is empty, so it can be destroyed.
    tx.moveCall({
        target: '0x2::bag::destroy_empty',
        typeArguments: [],
        arguments: [extractedNativeTokensBag],
    });

    // Extract the CoinManagerTreasuryCap.
    const coinManagerTreasuryCapObject = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::address_unlock_condition::unlock_alias_address_owned_coinmanager_treasury`,
        typeArguments: [foundryTokenType],
        arguments: [alias, tx.object(coinManagerTreasuryCapId)],
    });

    // Transfer the coin manager treasury cap.
    tx.transferObjects([coinManagerTreasuryCapObject], tx.pure.address(sender));

    // Transfer the alias asset.
    tx.transferObjects([alias], tx.pure.address(sender));

    // Set the gas budget for the transaction.
    tx.setGasBudget(10_000_000);

    // Sign and execute the transaction.
    const result = await iotaClient.signAndExecuteTransaction({ signer: keypair, transaction: tx });

    // Get the response of the transaction.
    const response = await iotaClient.waitForTransaction({ digest: result.digest });
    console.log(`Transaction digest: ${response.digest}`);
}

/**
 * Check if the object is a CoinManagerTreasuryCap.
 */
function isCoinManagerTreasuryCap(object: IotaObjectData): boolean {
    const splitType = object.type?.split("::");
    return splitType !== undefined &&
        splitType[0] === IOTA_FRAMEWORK_ADDRESS &&
        splitType[1] === COIN_MANAGER_MODULE_NAME &&
        splitType[2].split("<")[0] === COIN_MANAGER_TREASURY_CAP_STRUCT_NAME;
}

main().catch(error => {
    console.error(`Error: ${error.message}`);
});