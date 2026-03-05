import { Transaction } from '@iota/iota-sdk/transactions';

interface Transfer {
    to: string;
    amount: number;
}

declare function getTransfers(): Transfer[];
const transfers: Transfer[] = getTransfers();

const tx = new Transaction();

const coins = tx.splitCoins(
    tx.gas,
    transfers.map((transfer) => transfer.amount),
);

transfers.forEach((transfer, index) => {
    tx.transferObjects([coins[index]], transfer.to);
});
