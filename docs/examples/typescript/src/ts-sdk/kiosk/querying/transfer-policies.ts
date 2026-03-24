import { KioskClient } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const itemType = '0xAddress::hero::Hero';
const policies = await kioskClient.getTransferPolicies({ type: itemType });
console.log(policies);
