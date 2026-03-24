import { KioskClient } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';

const packageId = '0x...';
const myType = `${packageId}::my_module::MyStruct<${packageId}::my_coin_module::MyCoin>`;
const otherType = `${packageId}::other_module::OtherStruct<${packageId}::other_coin_module::OtherCoin>`;

const kioskClient = new KioskClient({
    client: new IotaClient({
        url: getFullnodeUrl(Network.Testnet),
    }),
    network: Network.Testnet,
});
