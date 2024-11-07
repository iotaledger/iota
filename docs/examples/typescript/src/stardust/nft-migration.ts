/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating the conversion of a stardust NFT into a custom user's
 * NFT. In order to work, it requires a network with test objects
 * generated from iota-genesis-builder/src/stardust/test_outputs.
 */

import {getFullnodeUrl, IotaClient} from "@iota/iota-sdk/client";
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
    // It is the same used in the Alias migration example.
    const customNftPackageId = await publishCustomNftPackage(iotaClient, keypair);

    // Get an NftOutput object id.
    const nftOutputObjectId = "0x6445847625cec7d1265ebb9d0da8050a2e43d2856c2746d3579df499a1a64226";
    // Get an NftOutput object.
    const nftOutputObject = await iotaClient.getObject({id: nftOutputObjectId, options: { showContent: true }});
    if (!nftOutputObject) {
        throw new Error("NFT output object not found");
    }

    // Create a PTB that extracts the stardust NFT from an NFTOutput and then calls
    // the `custom_nft::nft::convert` function for converting it into a custom NFT
    // of the just published package.
    const tx = new Transaction();
    const gasTypeTag = "0x2::iota::IOTA";
    const args = [tx.object(nftOutputObjectId)];
    // Call the nft_output::extract_assets function.
    const extractedNftOutputAssets = tx.moveCall({
        target: `${STARDUST_PACKAGE_ID}::nft_output::extract_assets`,
        typeArguments: [gasTypeTag],
        arguments: args,
    });

    // If the nft output can be unlocked, the command will be successful
    // and will return a `base_token` (i.e., IOTA) balance and a
    // `Bag` of native tokens and related nft object.
    const extractedBaseToken = extractedNftOutputAssets[0];
    const extractedNativeTokensBag = extractedNftOutputAssets[1];
    const nft = extractedNftOutputAssets[2];

    // Call the conversion function to create a custom nft from the stardust nft
    // asset.
    let customNft = tx.moveCall({
        target: `${customNftPackageId}::nft::convert`,
        typeArguments: [],
        arguments: [nft],
    });

    // Transfer the converted NFT.
    tx.transferObjects([customNft], tx.pure.address(sender));

    // Extract the IOTA balance.
    const iotaCoin = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: [gasTypeTag],
        arguments: [extractedBaseToken],
    });

    // Transfer the IOTA balance.
    tx.transferObjects([iotaCoin], tx.pure.address(sender));

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