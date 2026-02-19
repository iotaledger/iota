// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { resolve } from 'path';
import { describe, expect, it } from 'vitest';

import { bcs } from '../../src/bcs/index.js';
import {
    MoveAuthenticatorBuilder,
    MoveSigner,
} from '../../src/keypairs/move-authenticator/index.js';
import { Transaction } from '../../src/transactions/index.js';
import { publishPackage, setup } from './utils/setup.js';

const PACKAGE_PATH = resolve(__dirname, 'data/move-authenticator');

describe('MoveAuthenticator', () => {
    it('should publish, link auth, and execute a transaction with MoveAuthenticator', async () => {
        const toolbox = await setup();

        // 1. Publish the account package
        const { packageId, publishTxn } = await publishPackage(PACKAGE_PATH, toolbox);

        const accountChange = publishTxn.objectChanges?.find(
            (c) => c.type === 'created' && c.objectType.endsWith('::account::Account'),
        );
        const metadataChange = publishTxn.objectChanges?.find(
            (c) => c.type === 'created' && c.objectType.includes('PackageMetadataV1'),
        );
        expect(accountChange?.type).toBe('created');
        expect(metadataChange?.type).toBe('created');

        const accountId = accountChange!.type === 'created' ? accountChange!.objectId : '';
        const metadataId = metadataChange!.type === 'created' ? metadataChange!.objectId : '';

        // 2. Link the authenticator function
        const linkTx = new Transaction();
        linkTx.moveCall({
            target: `${packageId}::account::link_auth`,
            arguments: [
                linkTx.object(accountId),
                linkTx.object(metadataId),
                linkTx.pure.string('account'),
                linkTx.pure.string('authenticate'),
            ],
        });

        const linkResult = await toolbox.client.signAndExecuteTransaction({
            transaction: linkTx,
            signer: toolbox.keypair,
        });
        const linkConfirmed = await toolbox.client.waitForTransaction({
            digest: linkResult.digest,
            options: { showEffects: true },
        });
        expect(linkConfirmed.effects?.status.status).toEqual('success');

        // 3. Build a MoveSigner from the account
        const authBuilder = new MoveAuthenticatorBuilder(accountId).addPure(
            bcs.string().serialize('rustisbetterthanjavascript').toBytes(),
        );
        const moveSigner = new MoveSigner(await authBuilder.finish(toolbox.client));
        const aaAddress = moveSigner.getPublicKey().toIotaAddress();

        expect(moveSigner.getKeyScheme()).toBe('MoveAuthenticator');

        // 4. Fund the AA address
        const fundTx = new Transaction();
        const [fundCoin] = fundTx.splitCoins(fundTx.gas, [400_000_000]);
        fundTx.transferObjects([fundCoin], aaAddress);

        const fundResult = await toolbox.client.signAndExecuteTransaction({
            transaction: fundTx,
            signer: toolbox.keypair,
        });
        await toolbox.client.waitForTransaction({ digest: fundResult.digest });

        // 5. Execute a transfer from the AA address
        const transferTx = new Transaction();
        const [coin] = transferTx.splitCoins(transferTx.gas, [1000]);
        transferTx.transferObjects([coin], toolbox.address());
        transferTx.setSender(aaAddress);

        const builtTx = await transferTx.build({ client: toolbox.client });
        const result = await toolbox.client.signAndExecuteTransaction({
            signer: moveSigner,
            transaction: builtTx,
        });

        const confirmed = await toolbox.client.waitForTransaction({
            digest: result.digest,
            options: { showEffects: true },
        });
        expect(confirmed.effects?.status.status).toEqual('success');
    });
});
