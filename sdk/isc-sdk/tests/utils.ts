import { IotaClient } from '@iota/iota-sdk/client';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';

export async function requestFunds(
    client: IotaClient,
    faucetUrl: string,
    recipientAddress: string,
) {
    const keypair = new Ed25519Keypair();
    const address = keypair.toIotaAddress();

    await requestIotaFromFaucetV0({
        host: faucetUrl,
        recipient: address,
    });

    const transaction = new Transaction();
    const [coin] = transaction.splitCoins(transaction.gas, [9]);
    transaction.transferObjects([coin], recipientAddress);
    transaction.setSender(address);

    await transaction.build({ client });

    await client.signAndExecuteTransaction({
        signer: keypair,
        transaction,
    });
}
