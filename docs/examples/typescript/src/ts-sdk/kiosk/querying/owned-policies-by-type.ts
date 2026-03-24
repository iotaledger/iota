import { KioskClient } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const address = '0xAddress';
const type = '0xbe01d0594bedbce45c0e08c7374b03bf822e9b73cd7d555bf39c39bbf09d23a9::hero::Hero';

const policies = await kioskClient.getOwnedTransferPoliciesByType({ address, type });
