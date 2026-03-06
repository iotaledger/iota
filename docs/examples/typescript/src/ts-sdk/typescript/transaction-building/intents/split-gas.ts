import { Transaction } from '@iota/iota-sdk/transactions';

const recipient = '0x0';

const tx = new Transaction();

const [coin] = tx.splitCoins(tx.gas, [100]);

tx.transferObjects([coin], recipient);
