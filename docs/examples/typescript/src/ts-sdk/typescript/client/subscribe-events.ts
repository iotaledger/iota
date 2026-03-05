import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

const client = new IotaClient({
    url: getFullnodeUrl('testnet'),
});

const unsubscribe = await client.subscribeEvent({
    filter: {
        Sender: '<SENDER_ADDRESS>',
    },
    onMessage(event) {
        // handle subscription notification message here. This function is called once per subscription message.
    },
});

await unsubscribe();
