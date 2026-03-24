import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

const [coin] = tx.splitCoins(tx.gas, [100]);
tx.transferObjects([coin], '0xSomeIotaAddress');
