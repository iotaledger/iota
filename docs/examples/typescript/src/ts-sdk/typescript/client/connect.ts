import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

const rpcUrl = getFullnodeUrl('devnet');

const client = new IotaClient({ url: rpcUrl });

await client.getCoins({
    owner: '<OWNER_ADDRESS>',
});
