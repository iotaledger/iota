// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createMessage, type Message } from '_src/shared/messaging/messages';
import {
    isMethodPayload,
    type MethodPayload,
} from '_src/shared/messaging/messages/payloads/methodPayload';

import { type UiConnection } from '../connections/uiConnection';
import { getDB } from '../db';
import { AccountSourceType, type AccountSourceSerialized } from './accountSource';
import { MnemonicAccountSource } from './mnemonicAccountSource';
import { SeedAccountSource } from './seedAccountSource';
import { toEntropy } from '_src/shared/utils';
import { KeystoneAccountSource } from './keystoneAccountSource';
import { accountSourcesEvents } from './events';

function toAccountSource(accountSource: AccountSourceSerialized) {
    if (MnemonicAccountSource.isOfType(accountSource)) {
        return new MnemonicAccountSource(accountSource.id);
    }
    if (SeedAccountSource.isOfType(accountSource)) {
        return new SeedAccountSource(accountSource.id);
    }
    if (KeystoneAccountSource.isOfType(accountSource)) {
        return new KeystoneAccountSource(accountSource.id);
    }
    throw new Error(`Unknown account source of type ${accountSource.type}`);
}

export async function getAccountSources(filter?: { type: AccountSourceType }) {
    const db = await getDB();
    return (
        filter?.type
            ? await db.accountSources.where('type').equals(filter.type).sortBy('createdAt')
            : await db.accountSources.toCollection().sortBy('createdAt')
    ).map(toAccountSource);
}

export async function getAccountSourceByID(id: string) {
    const serializedAccountSource = await (await getDB()).accountSources.get(id);
    if (!serializedAccountSource) {
        return null;
    }
    return toAccountSource(serializedAccountSource);
}

export async function getAllSerializedUIAccountSources() {
    return Promise.all(
        (await getAccountSources()).map((anAccountSource) => anAccountSource.toUISerialized()),
    );
}

async function createAccountSource({ type, params }: MethodPayload<'createAccountSource'>['args']) {
    const { password } = params;
    let accountSource;
    switch (type) {
        case AccountSourceType.Mnemonic:
            const entropy = params.entropy;
            accountSource = await MnemonicAccountSource.save(
                await MnemonicAccountSource.createNew({
                    password,
                    entropyInput: entropy ? toEntropy(entropy) : undefined,
                }),
            );
            break;
        case AccountSourceType.Seed:
            accountSource = await SeedAccountSource.save(
                await SeedAccountSource.createNew({
                    password,
                    seed: params.seed,
                }),
            );
            break;
        case AccountSourceType.Keystone:
            accountSource = await KeystoneAccountSource.save(
                await KeystoneAccountSource.createNew({
                    password,
                    masterFingerprint: params.masterFingerprint,
                }),
            );
            break;
        default: {
            throw new Error(`Unknown Account source type ${type}`);
        }
    }
    return accountSource.toUISerialized();
}

export async function accountSourcesHandleUIMessage(msg: Message, uiConnection: UiConnection) {
    const { payload } = msg;
    if (isMethodPayload(payload, 'createAccountSource')) {
        uiConnection.send(
            createMessage<MethodPayload<'accountSourceCreationResponse'>>(
                {
                    method: 'accountSourceCreationResponse',
                    type: 'method-payload',
                    args: { accountSource: await createAccountSource(payload.args) },
                },
                msg.id,
            ),
        );
        accountSourcesEvents.emit('accountSourcesChanged');
        return true;
    }
    if (isMethodPayload(payload, 'unlockAccountSource')) {
        const { id, password } = payload.args;
        if (!password) {
            throw new Error('Missing password');
        }
        const accountSource = await getAccountSourceByID(id);
        if (!accountSource) {
            throw new Error('Account source not found');
        }
        await accountSource.unlock(password);
        uiConnection.send(createMessage({ type: 'done' }, msg.id));
        accountSourcesEvents.emit('accountSourcesChanged');
        return true;
    }
    if (isMethodPayload(payload, 'getAccountSourceEntropy')) {
        const accountSource = await getAccountSourceByID(payload.args.accountSourceID);
        if (!accountSource) {
            throw new Error('Account source not found');
        }
        if (!(accountSource instanceof MnemonicAccountSource)) {
            throw new Error('Invalid account source type');
        }
        uiConnection.send(
            createMessage<MethodPayload<'getAccountSourceEntropyResponse'>>(
                {
                    type: 'method-payload',
                    method: 'getAccountSourceEntropyResponse',
                    args: { entropy: await accountSource.getEntropy(payload.args.password) },
                },
                msg.id,
            ),
        );
        return true;
    }
    if (isMethodPayload(payload, 'getAccountSourceSeed')) {
        const accountSource = await getAccountSourceByID(payload.args.accountSourceID);
        if (!accountSource) {
            throw new Error('Account source not found');
        }
        if (!(accountSource instanceof SeedAccountSource)) {
            throw new Error('Invalid account source type');
        }
        uiConnection.send(
            createMessage<MethodPayload<'getAccountSourceSeedResponse'>>(
                {
                    type: 'method-payload',
                    method: 'getAccountSourceSeedResponse',
                    args: { seed: await accountSource.getSeed(payload.args.password) },
                },
                msg.id,
            ),
        );
        return true;
    }
    if (isMethodPayload(payload, 'verifyPasswordRecoveryData')) {
        const { accountSourceID, type } = payload.args.data;
        const accountSource = await getAccountSourceByID(accountSourceID);
        if (!accountSource) {
            throw new Error('Account source not found');
        }
        if (
            !(accountSource instanceof MnemonicAccountSource) &&
            !(accountSource instanceof SeedAccountSource)
        ) {
            throw new Error('Invalid account source type');
        }
        if (type === AccountSourceType.Mnemonic) {
            await accountSource.verifyRecoveryData(payload.args.data.entropy);
        }
        if (type === AccountSourceType.Seed) {
            await accountSource.verifyRecoveryData(payload.args.data.seed);
        }
        uiConnection.send(createMessage({ type: 'done' }, msg.id));
        return true;
    }
    return false;
}
