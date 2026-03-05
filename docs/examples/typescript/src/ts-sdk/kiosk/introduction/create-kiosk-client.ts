import { KioskClient } from '@iota/kiosk';
import { getFullnodeUrl, IotaClient, Network } from '@iota/iota-sdk/client';

const client = new IotaClient({ url: getFullnodeUrl(Network.Testnet) });

const kioskClient = new KioskClient({
    client,
    network: Network.Testnet,
});
