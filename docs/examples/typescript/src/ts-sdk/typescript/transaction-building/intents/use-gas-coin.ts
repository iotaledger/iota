import { coinWithBalance, Transaction } from '@iota/iota-sdk/transactions';

declare const recipient: string;

const tx = new Transaction();
tx.transferObjects([coinWithBalance({ balance: 100, useGasCoin: false })], recipient);
