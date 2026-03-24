import { getFullnodeUrl, IotaClient, IotaHTTPTransport } from '@iota/iota-sdk/client';
import { WebSocket } from 'ws';

new IotaClient({
    transport: new IotaHTTPTransport({
        url: getFullnodeUrl('testnet'),
        WebSocketConstructor: WebSocket as never,
    }),
});
