import { Transaction } from '@iota/iota-sdk/transactions';

declare const address: string;

const tx = new Transaction();

const [coin] = tx.splitCoins(tx.gas, [100]);
tx.transferObjects([coin], address);
