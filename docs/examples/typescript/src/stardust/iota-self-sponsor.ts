/** Copyright (c) 2024 IOTA Stiftung
 * SPDX-License-Identifier: Apache-2.0
 *
 * Example demonstrating the self-sponsor scenario for claiming an IOTA basic
 * output. In order to work, it requires a network with test objects
 * generated from iota-genesis-builder/src/stardust/test_outputs.
 */
import {getFullnodeUrl, IotaClient} from "@iota/iota-sdk/client";
import {Ed25519Keypair} from "@iota/iota-sdk/keypairs/ed25519";
import {Transaction} from "@iota/iota-sdk/transactions";

const MAIN_ADDRESS_MNEMONIC = "crazy drum raw dirt tooth where fee base warm beach trim rule sign silk fee fee dad large creek venue coin steel hub scale";
const STARDUST_PACKAGE_ID = "0x107a";

async function main() {
    // Build a client to connect to the local IOTA network.
    const iotaClient = new IotaClient({url: getFullnodeUrl('localnet')});

    // For this example we need to derive addresses that are at different
    // indexes and coin_types, one for sponsoring with IOTA coin type and one for
    // claiming the Basic Output with IOTA coin type.
    const sponsorDerivationPath = "m/44'/4218'/0'/0'/5'"
    const senderDerivationPath = "m/44'/4218'/0'/0'/50'"

    // Derive the address of the sponsor.
    const sponsorKeypair = Ed25519Keypair.deriveKeypair(MAIN_ADDRESS_MNEMONIC, sponsorDerivationPath);
    // Derive the address of the sender.
    const senderKeypair = Ed25519Keypair.deriveKeypair(MAIN_ADDRESS_MNEMONIC, senderDerivationPath);

    const sponsor = sponsorKeypair.toIotaAddress();
    const sender = senderKeypair.toIotaAddress();

    console.log(`Sender: ${sender}`);
    console.log(`Sponsor: ${sponsor}`);

    // This object id was fetched manually. It refers to a Basic Output object that
    // contains some Native Tokens.
    const basicOutputObjectId = "0xd0ed7f2c50366202585ebd52a38cde6a7a7282ef3f52db52c3ba87042bca6fba";
    // Get Basic Output object.
    const basicOutputObject = await iotaClient.getObject({id: basicOutputObjectId, options: { showBcs: true }});
    if (!basicOutputObject) {
        throw new Error("Basic output object not found");
    }

    // Create a PTB to claim the assets related to the basic output.
    const tx = new Transaction();
    // Extract the base token and native tokens bag.
    // Type argument for a Basic Output holding IOTA coin.
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
    // return a `base_token` balance and a `Bag` of native tokens.
    const extractedBaseToken = extractedBasicOutputAssets[0];
    const extractedNativeTokensBag = extractedBasicOutputAssets[1];

    // Delete the empty native tokens bag.
    tx.moveCall({
        target: `0x2::bag::destroy_empty`,
        typeArguments: [],
        arguments: [extractedNativeTokensBag],
    });

    // Create a coin from the extracted IOTA balance.
    const iotaCoin = tx.moveCall({
        target: '0x2::coin::from_balance',
        typeArguments: [gasTypeTag],
        arguments: [extractedBaseToken],
    });

    // Send back the base token coin to the user.
    tx.transferObjects([iotaCoin], tx.pure.address(sender));

    // Get a gas coin belonging to the sponsor.
    const sponsorGasObjects = await iotaClient.getCoins({ owner: sponsor });
    const sponsorGasCoin = sponsorGasObjects.data?.[0];

    if (!sponsorGasCoin) {
        throw new Error('No coins found for sponsor');
    }

    tx.setSender(sender);
    tx.setGasOwner(sponsor);
    // Set sponsor’s gas object to cover fees.
    tx.setGasPayment([{
        objectId: sponsorGasCoin.coinObjectId,
        version: sponsorGasCoin.version,
        digest: sponsorGasCoin.digest
    }]);
    tx.setGasBudget(10_000_000);

    // Sign the transaction with the sponsor and sender keypairs.
    const sponsorSignedTransaction = await tx.sign({ client: iotaClient, signer: sponsorKeypair });
    const senderSignedTransaction = await tx.sign({ client: iotaClient, signer: senderKeypair });

    // Build the transaction and execute it.
    const builtTransaction = await tx.build({ client: iotaClient });
    const result = await iotaClient.executeTransactionBlock({
        transactionBlock: builtTransaction,
        signature: [sponsorSignedTransaction.signature, senderSignedTransaction.signature]
    });

    // Get the response of the transaction.
    const response = await iotaClient.waitForTransaction({ digest: result.digest });
    console.log(`Transaction digest: ${response.digest}`);

}

main().catch(error => {
    console.error(`Error: ${error.message}`);
});