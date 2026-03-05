import { Transaction } from '@iota/iota-sdk/transactions';
import { IotaClient } from '@iota/iota-sdk/client';

const client = new IotaClient({ url: 'https://example.com' });

const tx = new Transaction();

await tx.build({ client });
