/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating the claim of a basic output.
 * In order to work, it requires a network with test objects
 * generated from iota-genesis-builder/src/stardust/test_outputs.
 */

import {getFullnodeUrl, IotaClient, IotaParsedData} from "@iota/iota-sdk/client";
import {Ed25519Keypair} from "@iota/iota-sdk/keypairs/ed25519";
import {Transaction} from "@iota/iota-sdk/transactions";

const MAIN_ADDRESS_MNEMONIC = "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum";
const STARDUST_PACKAGE_ID = "0x107a";

async function main() {
    // Build a client to connect to the local IOTA network.
    const iotaClient = new IotaClient({url: getFullnodeUrl('localnet')});

    // Derive the address of the first account.
    const keypair = Ed25519Keypair.deriveKeypair(MAIN_ADDRESS_MNEMONIC);
    const sender = keypair.toIotaAddress();
    console.log(`Sender address: ${sender}`);

    // This object id was fetched manually. It refers to a Basic Output object that
    // contains some Native Tokens.
    const basicOutputObjectId = "0xde09139ed46b9f5f876671e4403f312fad867c5ae5d300a252e4b6a6f1fa1fbd";

    // Get Basic Output object.
    const basicOutputObject = await iotaClient.getObject({id: basicOutputObjectId, options: { showContent: true }});
    if (!basicOutputObject) {
        throw new Error("Basic output object not found");
    }

    // Extract contents of the BasicOutput object.
    const moveObject = basicOutputObject.data?.content as IotaParsedData;
    if (moveObject.dataType != "moveObject") {
        throw new Error("BasicOutput is not a move object");
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

    // Create a PTB to claim the assets related to the basic output.
    const tx = new Transaction();

    ////// Command #1: extract the base token and native tokens bag.
    // Type argument for a Basic Output coming from the IOTA network, i.e., the IOTA
    // token or Gas type tag
    const gasTypeTag = "0x2::iota::IOTA";
    // Then pass the basic output object as input.
    const args = [tx.object(basicOutputObjectId)];
    // Finally call the basic_output::extract_assets function.
    const extractedBasicOutputAssets = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::basic_output::extract_assets`,
        typeArguments: [gasTypeTag],
        arguments: args,
    });

    // If the basic output can be unlocked, the command will be successful and will
    // return a `base_token` (i.e., IOTA) balance and a `Bag` of native tokens.
    const extractedBaseToken = extractedBasicOutputAssets[0];
    let extractedNativeTokensBag: any = extractedBasicOutputAssets[1];

    // Extract the IOTA balance.
    const iotaCoin = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: [gasTypeTag],
        arguments: [extractedBaseToken],
    });

    // Send back the base token coin to the user.
    tx.transferObjects([iotaCoin], tx.pure.address(sender));

    ////// Extract the native tokens from the Bag and send them to sender.
    for (const typeKey of dfTypeKeys) {
        // Type argument for a Native Token contained in the basic output bag.
        const typeArguments = [`0x${typeKey}`];
        // Then pass the bag and the receiver address as input.
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