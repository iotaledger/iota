import { KioskClient } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const address = '0xAddress';
const policies = await kioskClient.getOwnedTransferPolicies({ address });
console.log(policies);
