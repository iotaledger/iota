import { IotaClient, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const sender: string;
declare const sponsor: string;
declare const sponsorCoins: Awaited<ReturnType<IotaClient['getCoins']>>['data'];

const client = new IotaClient({ url: getFullnodeUrl('testnet') });

const tx = new Transaction();

// ... add some transactions...

const kindBytes = await tx.build({ client, onlyTransactionKind: true });

// construct a sponsored transaction from the kind bytes
const sponsoredtx = Transaction.fromKind(kindBytes);

// you can now set the sponsored transaction data that is required
sponsoredtx.setSender(sender);
sponsoredtx.setGasOwner(sponsor);
sponsoredtx.setGasPayment(
    sponsorCoins.map((coin) => ({
        objectId: coin.coinObjectId,
        version: coin.version,
        digest: coin.digest,
    })),
);
