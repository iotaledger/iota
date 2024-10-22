import { promises as fs } from 'fs';
import { join } from 'path';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import {IotaClient} from "@iota/iota-sdk/dist/cjs/client";
import { Transaction } from '@iota/iota-sdk/transactions';

const SPONSOR_ADDRESS_MNEMONIC = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";


/**
 * Utility function to fund an address with IOTA tokens.
 */
export async function fundAddress(
    iotaClient: IotaClient,
    recipient: string // IOTA address
): Promise<void> {
    try {
        // Derive the keypair and address from mnemonic.
        const keypair = Ed25519Keypair.deriveKeypair(SPONSOR_ADDRESS_MNEMONIC);
        const sponsor = keypair.toIotaAddress();
        console.log(`Sponsor address: ${sponsor}`);

        // Get a gas coin belonging to the sponsor.
        const gasObjects = await iotaClient.getCoins({ owner: sponsor });
        const gasCoin = gasObjects.data?.[0];

        if (!gasCoin) {
            throw new Error('No coins found for sponsor');
        }

        // Initialize a transaction.
        const tx = new Transaction();

        // Add a transfer to the transaction.
        tx.transferObjects([gasCoin.coinObjectId], recipient);

        // Set a gas budget for the transaction.
        tx.setGasBudget(10_000_000);

        // Sign and execute the transaction.
        const result = await iotaClient.signAndExecuteTransaction({ signer: keypair, transaction: tx });

        // Get the response of the transaction.
        const response = await iotaClient.waitForTransaction({ digest: result.digest });
        console.log(`Funding transaction digest: ${response.digest}`);

    } catch (error: any) {
        console.error(`Error funding address: ${error.message}`);
        throw error;
    }
}
