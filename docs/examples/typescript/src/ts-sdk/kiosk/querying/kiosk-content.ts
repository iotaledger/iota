import { KioskClient } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const id = '0xKioskId';

const res = await kioskClient.getKiosk({
    id,
    options: {
        withKioskFields: true,
        withListingPrices: true,
    },
});
console.log(res);
