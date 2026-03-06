import { KioskClient } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const type = '0xAddress::custom_extension::ACustomExtensionType';

const extension = await kioskClient.getKioskExtension({
    kioskId: '0xAKioskId',
    type,
});

console.log(extension);
