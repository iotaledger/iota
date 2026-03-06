import { KioskClient, TransferPolicyTransaction } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const tx = new Transaction();
const tpTx = new TransferPolicyTransaction({ kioskClient, transaction: tx });

tpTx.addPersonalKioskRule();
