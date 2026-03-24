import { KioskClient, TransferPolicyTransaction } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

// In a real app, use signAndExecuteTransaction from @iota/dapp-kit
async function signAndExecuteTransaction(_args: { tx: Transaction }): Promise<void> {}

const packageId = '0x0';

function percentageToBasisPoints(percentage: number): number {
    return percentage * 100;
}

const heroPolicyCaps = await kioskClient.getOwnedTransferPoliciesByType({
    type: `${packageId}::hero::Hero`,
    address: '0xConnectedAddress',
});

const tx = new Transaction();
const tpTx = new TransferPolicyTransaction({ kioskClient, transaction: tx, cap: heroPolicyCaps[0] });

tpTx
    .addFloorPriceRule(10n)
    .addLockRule()
    .addRoyaltyRule(percentageToBasisPoints(10), 0)
    .addPersonalKioskRule();

await signAndExecuteTransaction({ tx });
