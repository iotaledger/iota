import { getFullnodeUrl, IotaClient, IotaHTTPTransport } from '@iota/iota-sdk/client';

const client = new IotaClient({
    transport: new IotaHTTPTransport({
        url: 'https://my-custom-node.com/rpc',
        websocket: {
            reconnectTimeout: 1000,
            url: 'https://my-custom-node.com/websockets',
        },
        rpc: {
            headers: {
                'x-custom-header': 'custom value',
            },
        },
    }),
});
