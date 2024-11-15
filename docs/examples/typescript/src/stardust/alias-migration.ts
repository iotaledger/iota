/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating the conversion of a stardust Alias into a custom
 * user's NFT collections controller. In order to work, it requires a network
 * with test objects generated from
 * iota-genesis-builder/src/stardust/test_outputs.
 */

import {getFullnodeUrl, IotaClient, IotaParsedData} from "@iota/iota-sdk/client";
import {Ed25519Keypair} from "@iota/iota-sdk/keypairs/ed25519";
import {publishCustomNftPackage} from "../utils";
import {Transaction} from "@iota/iota-sdk/transactions";

const MAIN_ADDRESS_MNEMONIC = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";
const STARDUST_PACKAGE_ID = "0x107a";

async function main() {
    // Build a client to connect to the local IOTA network.
    const iotaClient = new IotaClient({url: getFullnodeUrl('localnet')});

    // Derive the address of the first account.
    const keypair = Ed25519Keypair.deriveKeypair(MAIN_ADDRESS_MNEMONIC);
    const sender = keypair.toIotaAddress();
    console.log(`Sender address: ${sender}`);

    // Publish the package of a custom NFT collection and then get the package id.
    // The custom NFT module is obtained from a Move example in the docs.
    // It is the same used in the Nft migration example.
    const customNftPackageId = await publishCustomNftPackage(iotaClient, keypair);

    // Get the AliasOutput object.
    const aliasOutputObjectId = "0x354a1864c8af23fde393f7603bc133f755a9405353b30878e41b929eb7e37554";
    const aliasOutputObject = await iotaClient.getObject({id: aliasOutputObjectId, options: { showContent: true }});
    if (!aliasOutputObject) {
        throw new Error("Alias output object not found");
    }

    // Extract contents of the AliasOutput object.
    const moveObject = aliasOutputObject.data?.content as IotaParsedData;
    if (moveObject.dataType != "moveObject") {
        throw new Error("AliasOutput is not a move object");
    }

    // Treat fields as key-value object.
    const fields = moveObject.fields as Record<string, any>;

    // Access fields by key
    // const id = fields['id'];                     // UID field
    // const balance = fields['balance'];           // Balance<T> field
    const nativeTokensBag = fields['native_tokens'];   // Bag field

    // Extract the keys of the native_tokens bag if it is not empty; the keys
    // are the type_arg of each native token, so they can be used later in the PTB.
    const dfTypeKeys: string[] = [];
    if (nativeTokensBag.fields.size > 0) {
        // Get the dynamic fields owned by the native tokens bag.
        const dynamicFieldPage = await iotaClient.getDynamicFields({
            parentId: nativeTokensBag.fields.id.id
        });

        // Extract the dynamic fields keys, i.e., the native token type.
        dynamicFieldPage.data.forEach(dynamicField => {
            if (typeof dynamicField.name.value === 'string') {
                dfTypeKeys.push(dynamicField.name.value);
            } else {
                throw new Error('Dynamic field key is not a string');
            }
        });
    }

    // Create a PTB that extracts the related stardust Alias from the AliasOutput
    // and then calls the
    // `custom_nft::collection::convert_alias_to_collection_controller_cap` function
    // to convert it into an NFT collection controller, create a collection and mint
    // a few NFTs.
    const tx = new Transaction();
    const gasTypeTag = "0x2::iota::IOTA";
    const args = [tx.object(aliasOutputObjectId)];

    // Call the nft_output::extract_assets function.
    const extractedAliasOutputAssets = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::alias_output::extract_assets`,
        typeArguments: [gasTypeTag],
        arguments: args,
    });

    // The alias output can always be unlocked by the governor address. So the
    // command will be successful and will return a `base_token` (i.e., IOTA)
    // balance, a `Bag` of the related native tokens and the related Alias object.
    // Extract contents.
    const extractedBaseToken = extractedAliasOutputAssets[0];
    let extractedNativeTokensBag: any = extractedAliasOutputAssets[1];
    const alias = extractedAliasOutputAssets[2];

    // Call the conversion function to create an NFT collection controller from the
    // extracted alias.
    let nftCollectionController = tx.moveCall({
        target: `${customNftPackageId}::collection::convert_alias_to_collection_controller_cap`,
        typeArguments: [],
        arguments: [alias],
    });

    // Create an NFT collection.
    let nftCollection = tx.moveCall({
        target: `${customNftPackageId}::collection::create_collection`,
        typeArguments: [],
        arguments: [nftCollectionController, tx.pure.string("Collection name")],
    });

    // Mint a collection-related NFT.
    const nftName = tx.pure.string("NFT name");
    const nftDescription = tx.pure.string("NFT description");
    const nftUrlVal = tx.pure.string("NFT URL");

    const nftUrl = tx.moveCall({
        target: `0x2::url::new_unsafe`,
        typeArguments: [],
        arguments: [nftUrlVal],
    });

    const nft = tx.moveCall({
        target: `${customNftPackageId}::nft::mint_collection_related`,
        typeArguments: [],
        arguments: [nftCollection, nftName, nftDescription, nftUrl],
    });

    // Transfer the NFT.
    tx.transferObjects([nft], tx.pure.address(sender));

    // Drop the NFT collection to make impossible to mint new related NFTs.
    tx.moveCall({
        target: `${customNftPackageId}::collection::drop_collection`,
        typeArguments: [],
        arguments: [nftCollectionController, nftCollection],
    });

    // Transfer the NFT collection controller.
    tx.transferObjects([nftCollectionController], tx.pure.address(sender));

    // Extract the IOTA balance.
    const iotaCoin = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: [gasTypeTag],
        arguments: [extractedBaseToken],
    });

    // Transfer the IOTA balance to the sender.
    tx.transferObjects([iotaCoin], tx.pure.address(sender));

    // Extract the native tokens from the bag.
    for (const typeKey of dfTypeKeys) {
        const typeArguments = [`0x${typeKey}`];
        const args = [extractedNativeTokensBag, tx.pure.address(sender)]

        extractedNativeTokensBag = tx.moveCall({
            target: `${STARDUST_PACKAGE_ID}::utilities::extract_and_send_to`,
            typeArguments: typeArguments,
            arguments: args,
        });
    }

    // Cleanup the bag by destroying it.
    tx.moveCall({
        target: `0x2::bag::destroy_empty`,
        typeArguments: [],
        arguments: [extractedNativeTokensBag],
    });

    // Set the gas budget for the transaction.
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