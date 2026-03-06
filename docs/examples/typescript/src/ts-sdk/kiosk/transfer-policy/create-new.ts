import { KioskClient, TransferPolicyTransaction } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

// In a real app, use signAndExecuteTransaction from @iota/dapp-kit
async function signAndExecuteTransaction(_args: { tx: Transaction }): Promise<void> {}

const heroPackageId = '0x0';

function percentageToBasisPoints(percentage: number): number {
    return percentage * 100;
}

const publisher = '0xPackagePublisherObject';
const tx = new Transaction();

const tpTx = new TransferPolicyTransaction({ kioskClient, transaction: tx });

await tpTx.create({
    type: `${heroPackageId}::hero::Hero`,
    publisher,
});

tpTx.addLockRule()
    .addFloorPriceRule(1000n)
    .addRoyaltyRule(percentageToBasisPoints(10), 100)
    .addPersonalKioskRule()
    .shareAndTransferCap('address_to_transfer_cap_to');

await signAndExecuteTransaction({ tx });
