import { Transaction } from '@iota/iota-sdk/transactions';

interface Transfer {
    to: string;
    amount: number;
}

function getTransfers(): Transfer[] {
    // In a real app, return transfers from your data source
    return [
        { to: '0x0', amount: 100 },
        { to: '0x1', amount: 200 },
    ];
}
const transfers: Transfer[] = getTransfers();

const tx = new Transaction();

const coins = tx.splitCoins(
    tx.gas,
    transfers.map((transfer) => transfer.amount),
);

transfers.forEach((transfer, index) => {
    tx.transferObjects([coins[index]], transfer.to);
});
