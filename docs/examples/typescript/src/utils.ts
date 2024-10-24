import * as path from 'path';
import {Ed25519Keypair} from '@iota/iota-sdk/keypairs/ed25519';
import {IotaClient, type IotaObjectChangePublished} from "@iota/iota-sdk/dist/cjs/client";
import {Transaction} from '@iota/iota-sdk/transactions';
import {execSync} from 'child_process';

const SPONSOR_ADDRESS_MNEMONIC = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";
const CUSTOM_NFT_PACKAGE_PATH = "../../move/custom_nft";
const IOTA_BIN = path.resolve(__dirname, '../../../../target/release/iota');


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

/**
 * Utility function to publish a custom NFT package found in the Move examples.
 */
export async function publishCustomNftPackage(
    iotaClient: IotaClient,
    keypair: Ed25519Keypair
): Promise<string> {
    try {
        // Compile the custom NFT package
        const packagePath = path.join(__dirname, CUSTOM_NFT_PACKAGE_PATH);
        return await publishPackage(iotaClient, keypair, packagePath);
    } catch (error) {
        console.error("Error publishing custom NFT package:", error);
        throw error;
    }
}


/**
 * Utility function to publish a package.
 */
async function publishPackage(iotaClient: IotaClient, keypair: Ed25519Keypair, packagePath: string): Promise<string> {
    const { modules, dependencies } = JSON.parse(
        execSync(
            `${IOTA_BIN} move build --dump-bytecode-as-base64 --path ${packagePath}`,
            { encoding: 'utf-8' },
        ),
    );

    const tx = new Transaction();
    const cap = tx.publish({
        modules,
        dependencies,
    });

    const sender = keypair.toIotaAddress();

    // Transfer the upgrade capability to the sender so they can upgrade the package later if they want.
    tx.transferObjects([cap], tx.pure.address(sender));

    const { digest } = await iotaClient.signAndExecuteTransaction({
        transaction: tx,
        signer: keypair,
    });

    const publishTxn = await iotaClient.waitForTransaction({
        digest: digest,
        options: { showObjectChanges: true, showEffects: true },
    });
    console.log(`Publish transaction digest: ${publishTxn.digest}`);

    // expect(publishTxn.effects?.status.status).toEqual('success');

    const packageId = ((publishTxn.objectChanges?.filter(
        (a) => a.type === 'published',
    ) as IotaObjectChangePublished[]) ?? [])[0]?.packageId.replace(/^(0x)(0+)/, '0x') as string;

    // expect(packageId).toBeTypeOf('string');

    console.info(`Published package ${packageId} from address ${sender}}`);

    return packageId;
}

