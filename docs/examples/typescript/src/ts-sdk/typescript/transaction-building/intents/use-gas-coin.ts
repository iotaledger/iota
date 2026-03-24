import { coinWithBalance, Transaction } from '@iota/iota-sdk/transactions';

const recipient = '0x0';

const tx = new Transaction();
tx.transferObjects([coinWithBalance({ balance: 100, useGasCoin: false })], recipient);
