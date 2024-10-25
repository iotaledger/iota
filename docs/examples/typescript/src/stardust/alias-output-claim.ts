/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating the claim of an alias output.
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

    // Derive keypair from mnemonic.
    const keypair = Ed25519Keypair.deriveKeypair(MAIN_ADDRESS_MNEMONIC);
    const sender = keypair.toIotaAddress();
    console.log(`Sender address: ${sender}`);

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
        const dynamicFieldPage = await iotaClient.getDynamicFields({
            parentId: nativeTokensBag.fields.id.id
        });

        dynamicFieldPage.data.forEach(dynamicField => {
            if (typeof dynamicField.name.value === 'string') {
                dfTypeKeys.push(dynamicField.name.value);
            } else {
                throw new Error('Dynamic field key is not a string');
            }
        });
    }

    // Create a PTB to claim the assets related to the alias output.
    const tx = new Transaction();
    const gasTypeTag = "0x2::iota::IOTA";
    const args = [tx.object(aliasOutputObjectId)];
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


main().catch(error => {
    console.error(`Error: ${error.message}`);
});