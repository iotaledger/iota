// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { decrypt, encrypt } from '_src/shared/cryptography/keystore';
import { sha256 } from '@noble/hashes/sha256';
import { bytesToHex } from '@noble/hashes/utils';
import Dexie from 'dexie';

import { getAccountSources } from '.';
import { setupAutoLockAlarm } from '../autoLockAccounts';
import { backupDB, getDB } from '../db';
import {
    AccountSource,
    AccountSourceType,
    type AccountSourceSerialized,
    type AccountSourceSerializedUI,
} from './accountSource';

interface KeystoneAccountSourceSerialized extends AccountSourceSerialized {
    type: AccountSourceType.Keystone;
    encryptedData: string;
    // hash of entropy to be used for comparing sources (even when locked)
    sourceHash: string;
    masterFingerprint: string;
}

interface KeystoneAccountSourceSerializedUI extends AccountSourceSerializedUI {
    type: AccountSourceType.Keystone;
}

export class KeystoneAccountSource extends AccountSource<KeystoneAccountSourceSerialized> {
    static async createNew({
        password,
        masterFingerprint,
    }: {
        password: string;
        masterFingerprint: string;
    }) {
        // This way the UI can always know the source id by just knowing the master fingerprint
        const id = `keystone-${masterFingerprint}`;
        const dataSerialized: KeystoneAccountSourceSerialized = {
            id,
            type: AccountSourceType.Keystone,
            encryptedData: await encrypt(password, {}),
            sourceHash: bytesToHex(sha256(id)),
            createdAt: Date.now(),
            masterFingerprint,
        };
        const allAccountSources = await getAccountSources();
        for (const anAccountSource of allAccountSources) {
            if (
                anAccountSource instanceof KeystoneAccountSource &&
                (await anAccountSource.sourceHash) === dataSerialized.sourceHash
            ) {
                throw new Error('Keystone account source already exists');
            }
        }
        return dataSerialized;
    }

    static isOfType(
        serialized: AccountSourceSerialized,
    ): serialized is KeystoneAccountSourceSerialized {
        return serialized.type === AccountSourceType.Keystone;
    }

    static async save(serialized: KeystoneAccountSourceSerialized) {
        await (await Dexie.waitFor(getDB())).accountSources.put(serialized);
        await backupDB();
        return new KeystoneAccountSource(serialized.id);
    }

    constructor(id: string) {
        super({ type: AccountSourceType.Keystone, id });
    }

    async isLocked() {
        return (await this.getEphemeralValue()) === null;
    }

    async unlock(password: string) {
        await this.setEphemeralValue(await this.#decryptStoredData(password));
        await setupAutoLockAlarm();
    }

    async verifyPassword(password: string) {
        const { encryptedData } = await this.getStoredData();
        await decrypt(password, encryptedData);
    }

    async lock() {
        const isLocked = await this.isLocked();
        if (!isLocked) {
            await this.clearEphemeralValue();
        }
    }

    async toUISerialized(): Promise<KeystoneAccountSourceSerializedUI> {
        const { type } = await this.getStoredData();
        return {
            id: this.id,
            type,
            isLocked: await this.isLocked(),
        };
    }

    get masterFingerprint() {
        return this.getStoredData().then(({ masterFingerprint }) => masterFingerprint);
    }

    get sourceHash() {
        return this.getStoredData().then(({ sourceHash }) => sourceHash);
    }

    async #decryptStoredData(password: string) {
        const { encryptedData } = await this.getStoredData();
        return decrypt(password, encryptedData);
    }
}
