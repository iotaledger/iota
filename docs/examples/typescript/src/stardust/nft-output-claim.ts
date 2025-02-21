/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating the claim of an NFT output.
 * In order to work, it requires a network with test objects
 * generated from iota-genesis-builder/src/stardust/test_outputs.
 */
import {getFullnodeUrl, IotaClient, IotaParsedData} from "@iota/iota-sdk/client";
import {Ed25519Keypair} from "@iota/iota-sdk/keypairs/ed25519";
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

    // Get the NFTOutput object.
    const nftOutputObjectId = "0xad87a60921c62f84d57301ea127d1706b406cde5ec6fa4d3af2a80f424fab93a";
    const nftOutputObject = await iotaClient.getObject({
        id: nftOutputObjectId,
        options: {
            showContent: true,
            showBcs: true
        }
    });
    if (!nftOutputObject) {
        throw new Error("NFT output object not found");
    }

    // Extract contents of the NftOutput object.
    const moveObject = nftOutputObject.data?.content as IotaParsedData;
    if (moveObject.dataType != "moveObject") {
        throw new Error("NftOutput is not a move object");
    }

    // Treat fields as key-value object.
    const fields = moveObject.fields as Record<string, any>;

    // Access fields by key
    const nativeTokensBag = fields['native_tokens'];

    // Extract the keys of the native_tokens bag if it is not empty; the keys
    // are the type_arg of each native token, so they can be used later in the PTB.
    const dfTypeKeys: string[] = [];
    if (nativeTokensBag.fields.size > 0) {
        // Get the dynamic fieldss of the native tokens bag.
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

    const tx = new Transaction();
    // Extract nft assets(base token, native tokens bag, nft asset itself).
    const gasTypeTag = "0x2::iota::IOTA";
    const args = [tx.object(nftOutputObjectId)];
    // Finally call the nft_output::extract_assets function.
    const extractedNftOutputAssets = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::nft_output::extract_assets`,
        typeArguments: [gasTypeTag],
        arguments: args,
    });

    // If the nft output can be unlocked, the command will be successful and will
    // return a `base_token` (i.e., IOTA) balance and a `Bag` of native tokens and
    // related nft object.
    const extractedBaseToken = extractedNftOutputAssets[0];
    let extractedNativeTokensBag: any = extractedNftOutputAssets[1];
    const nft = extractedNftOutputAssets[2];

    // Extract the IOTA balance.
    const iotaCoin = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: [gasTypeTag],
        arguments: [extractedBaseToken],
    });

    // Transfer the IOTA balance.
    tx.transferObjects([iotaCoin], tx.pure.address(sender));

    // Extract the native tokens from the bag.
    for (const typeKey of dfTypeKeys) {
        const typeArguments = [`0x${typeKey}`];
        // Then pass the bag and the receiver address as input.
        const args = [extractedNativeTokensBag, tx.pure.address(sender)]

        // Extract native tokens from the bag.
        // Extract native token balance.
        // Transfer native token balance.
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

    // Transfer the nft asset.
    tx.transferObjects([nft], tx.pure.address(sender));

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