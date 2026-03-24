import { coinWithBalance, Transaction } from '@iota/iota-sdk/transactions';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const keypair = new Ed25519Keypair();
const recipient = '0x0';

const tx = new Transaction();

tx.setSender(keypair.toIotaAddress());

tx.transferObjects(
    [
        coinWithBalance({ balance: 100 }),
        coinWithBalance({ balance: 100, type: '0x123::foo:Bar' }),
    ],
    recipient,
);
