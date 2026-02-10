// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Move Authenticator example using an already-deployed account object.
 *
 * Assumes the `account` package has been published and `link_auth` has
 * already been called. Only the account object ID is needed.
 */

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { bcs } from '@iota/iota-sdk/bcs';
import { MoveAuthenticatorBuilder, MoveSigner } from '@iota/iota-sdk/keypairs/move-authenticator';

// Replace this with your own.
const ACCOUNT_ID = '0xf9d1fc0438de7c776210ec6d5ebee054f1f23c8015bcfda5aa7ba71b3e3c3a13';

const client = new IotaClient({ url: getFullnodeUrl('devnet') });

const authBuilder = new MoveAuthenticatorBuilder(ACCOUNT_ID).addPure(
    bcs.string().serialize('hello').toBytes(),
);
const moveSigner = new MoveSigner(await authBuilder.finish(client));
const aaAddress = moveSigner.getPublicKey().toIotaAddress();
console.log('Account-abstracted address:', aaAddress);

// Demo recipient
const recipient = '0x14abc6dfbf9ae91106ccc21b1a7839704cc9932a8ab571b7f60a2894cea219e7';
const tx = new Transaction();
const [coin] = tx.splitCoins(tx.gas, [1000]);
tx.transferObjects([coin], recipient);
tx.setSender(aaAddress);

const builtTx = await tx.build({ client });
const result = await client.signAndExecuteTransaction({
    signer: moveSigner,
    transaction: builtTx,
});

console.log('Transfer digest:', result.digest);
await client.waitForTransaction({ digest: result.digest });
console.log('Transfer confirmed.');
