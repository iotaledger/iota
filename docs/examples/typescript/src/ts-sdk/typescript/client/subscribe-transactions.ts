import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

const client = new IotaClient({
    url: getFullnodeUrl('testnet'),
});

const unsubscribe = await client.subscribeTransaction({
    filter: {
        FromAddress: '<IOTA_ADDRESS>',
    },
    onMessage(event) {
        // This function is called once per transaction.
    },
});

await unsubscribe();
