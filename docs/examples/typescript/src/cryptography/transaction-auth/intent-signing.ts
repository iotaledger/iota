import { messageWithIntent } from '@iota/iota-sdk/cryptography';

declare const transactionBytes: Uint8Array;
declare function sign(data: Uint8Array): Promise<Uint8Array>;

const intentMessage = messageWithIntent('TransactionData', transactionBytes);
const signature = await sign(intentMessage);
