// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating how to unlock an output owned by an alias output.
//! In order to work, it requires a network with test objects
//! generated from iota-genesis-builder/src/stardust/test_outputs.

import {getFullnodeUrl, IotaClient} from "@iota/iota-sdk/client";
import {Ed25519Keypair} from "@iota/iota-sdk/keypairs/ed25519";
import {fundAddress} from "../utils";
import {bcs} from '@iota/iota-sdk/bcs'
import {Transaction} from "@iota/iota-sdk/transactions";

const MAIN_ADDRESS_MNEMONIC = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";

async function main() {
    // 1. Build a client to connect to the local IOTA network.

    // Build a client to connect to the local IOTA network.
    const iotaClient = new IotaClient({url: getFullnodeUrl('localnet')});

    // 2. Derive keypair from mnemonic.

    // For this example we need to derive an address that is not at index 0. This
    // because we need an alias output that owns an Nft Output. In this case, we can
    // derive the address index "/2'" of the "/0'" account.
    const derivationPath = "m/44'/4218'/0'/0'/2'"

    const keypair = Ed25519Keypair.deriveKeypair(MAIN_ADDRESS_MNEMONIC, derivationPath);

    const sender = keypair.toIotaAddress();
    console.log(`Sender: ${sender}`);

    // 3. Fund address from sponsor (in utils.ts)
    await fundAddress(iotaClient, sender);

    // 4. Get a gas coin
    // We actually don't need to do this, because the execution uses tx.gas by default.

    // 5. Fetch alias_output_object_id and its object
    const aliasOutputObjectId = "0x3b35e67750b8e4ccb45b2fc4a6a26a6d97e74c37a532f17177e6324ab93eaca6";
    const aliasOutputObject = await iotaClient.getObject({id: aliasOutputObjectId});
    if (!aliasOutputObject) {
        throw new Error("Alias output object not found");
    }

    // 6. Get the dynamic field of the object
    // Define the dynamic field name for the Alias object "alias", of type vector<u8>.

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

    // 7. Get NftOutput objects owned by the alias object

    // Some objects are owned by the Alias object. In this case we filter them by
    // type using the NftOutput type.

    // Fetch the owned objects with the specified filter.

    // The StructTag is defined as follows:
    // StructTag {
    //     address: STARDUST_PACKAGE_ID.into(),
    //         module: NFT_OUTPUT_MODULE_NAME.to_owned(),
    //         name: NFT_OUTPUT_STRUCT_NAME.to_owned(),
    //         type_params: vec![type_param],
    // }
    //
    // with:
    const STARDUST_PACKAGE_ID = "0x107a";
    const NFT_OUTPUT_MODULE_NAME = "nft_output";
    const NFT_OUTPUT_STRUCT_NAME = "NftOutput";

    // todo: use this instead of the hardcoded nftOutputStructTag
    // const gasTypeTag: TypeTag = {
    //     struct: {
    //         address: "0x2",
    //         module: "iota",
    //         name: "IOTA",
    //         typeParams: [],
    //     },
    // };
    //
    // const nftOutputStructTag = bcs.StructTag.serialize({
    //     address: STARDUST_PACKAGE_ID,
    //     module: NFT_OUTPUT_MODULE_NAME,
    //     name: NFT_OUTPUT_STRUCT_NAME,
    //     typeParams: [gasTypeTag],
    // });

    const gasTypeTag = "0x2::iota::IOTA";
    const nftOutputStructTag = `${STARDUST_PACKAGE_ID}::${NFT_OUTPUT_MODULE_NAME}::${NFT_OUTPUT_STRUCT_NAME}<${gasTypeTag}>`;

    const ownedObjects = await iotaClient.getOwnedObjects({
        owner: aliasObjectId ? aliasObjectId.toString() : "",
        filter: {
            StructType: nftOutputStructTag,
        },
    });

    // Get the first NftOutput found
    const nftOutputObjectOwnedByAlias = ownedObjects.data?.[0]?.data;
    if (!nftOutputObjectOwnedByAlias) {
        throw new Error("Owned NftOutput not found");
    }

    const nftOutputObjectId = nftOutputObjectOwnedByAlias.objectId;

    // 8. Create the ptb
    const tx = new Transaction();

    const typeArgs = [gasTypeTag];
    const args = [tx.object(aliasOutputObjectId)]
    const extractedAliasOutputAssets = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::alias_output::extract_assets`,
        typeArguments: typeArgs,
        arguments: args,
    });

    // Extract contents.
    const extractedBaseToken = extractedAliasOutputAssets[0];
    const extractedNativeTokensBag = extractedAliasOutputAssets[1];
    const alias = extractedAliasOutputAssets[2];

    // Extract the IOTA balance.
    const iotaCoin = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: typeArgs,
        arguments: [extractedBaseToken],
    });

    // Transfer the IOTA balance to the sender.
    tx.transferObjects([iotaCoin], tx.pure.address(sender));

    // Cleanup the bag by destroying it.
    tx.moveCall({
        target: '0x2::bag::destroy_empty',
        typeArguments: [],
        arguments: [extractedNativeTokensBag],
    });

    const aliasArg = alias;
    const nftOutputArg = tx.object(nftOutputObjectId);

    const nftOutput = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::address_unlock_condition::unlock_alias_address_owned_nft`,
        typeArguments: typeArgs,
        arguments: [aliasArg, nftOutputArg],
    });

    tx.transferObjects([alias], tx.pure.address(sender));

    // Extract the assets from the NftOutput.
    const extractedAssets = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::nft_output::extract_assets`,
        typeArguments: typeArgs,
        arguments: [nftOutput],
    });

    const extractedBaseToken2 = extractedAssets[0];
    const extractedNativeTokensBag2 = extractedAssets[1];
    const nftAsset = extractedAssets[2];

    const iotaCoin2 = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: typeArgs,
        arguments: [extractedBaseToken2],
    });

    tx.transferObjects([iotaCoin2], tx.pure.address(sender));

    tx.moveCall({
        target: '0x2::bag::destroy_empty',
        typeArguments: [],
        arguments: [extractedNativeTokensBag2],
    });

    tx.transferObjects([nftAsset], tx.pure.address(sender));

    // 9. Submit the ptb
    tx.setGasBudget(10_000_000);

    // Sign and execute the transaction.
    const result = await iotaClient.signAndExecuteTransaction({ signer: keypair, transaction: tx });

    // Get the response of the transaction.
    const response = await iotaClient.waitForTransaction({ digest: result.digest });
    console.log(`Transaction digest: ${response.digest}`);
}

main().catch(error => {
    console.error(`Error: ${error.message}`);
});