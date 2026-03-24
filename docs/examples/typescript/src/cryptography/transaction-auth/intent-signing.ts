import { messageWithIntent } from '@iota/iota-sdk/cryptography';

const transactionBytes = new Uint8Array(32);
async function sign(_data: Uint8Array): Promise<Uint8Array> {
    // In a real app, sign with a keypair: keypair.signTransaction(data)
    return new Uint8Array(64);
}

const intentMessage = messageWithIntent('TransactionData', transactionBytes);
const signature = await sign(intentMessage);
