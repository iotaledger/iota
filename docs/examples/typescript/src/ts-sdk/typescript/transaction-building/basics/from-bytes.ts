import { Transaction } from '@iota/iota-sdk/transactions';

declare function getTransactionBytesFromSomewhere(): Uint8Array;
const bytes = getTransactionBytesFromSomewhere();
const tx = Transaction.from(bytes);
