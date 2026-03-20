// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Account,
    AccountType,
    type PasswordUnlockableAccount,
    type SerializedAccount,
    type SerializedUIAccount,
} from './account';
import { KeystoneAccountSource } from '../account-sources/keystoneAccountSource';

export interface KeystoneAccountSerialized extends SerializedAccount {
    type: AccountType.KeystoneDerived;
    derivationPath: string;
    sourceID: string;
}

export interface KeystoneAccountSerializedUI extends SerializedUIAccount {
    type: AccountType.KeystoneDerived;
    derivationPath: string;
    sourceID: string;
    masterFingerprint: string;
}

export function isKeystoneAccountSerializedUI(
    account: SerializedUIAccount,
): account is KeystoneAccountSerializedUI {
    return account.type === AccountType.KeystoneDerived;
}

export class KeystoneAccount
    extends Account<KeystoneAccountSerialized>
    implements PasswordUnlockableAccount
{
    readonly unlockType = 'password';

    static async createNew({
        address,
        publicKey,
        sourceID,
        derivationPath,
    }: {
        address: string;
        publicKey: string | null;
        derivationPath: string;
        sourceID: string;
    }): Promise<Omit<KeystoneAccountSerialized, 'id'>> {
        return {
            type: AccountType.KeystoneDerived,
            address,
            publicKey,
            derivationPath,
            lastUnlockedOn: null,
            selected: false,
            nickname: null,
            createdAt: Date.now(),
            sourceID,
        };
    }

    static isOfType(serialized: SerializedAccount): serialized is KeystoneAccountSerialized {
        return serialized.type === AccountType.KeystoneDerived;
    }

    constructor({ id, cachedData }: { id: string; cachedData?: KeystoneAccountSerialized }) {
        super({ type: AccountType.KeystoneDerived, id, cachedData });
    }

    get derivationPath() {
        return this.getCachedData().then(({ derivationPath }) => derivationPath);
    }

    get sourceID() {
        return this.getCachedData().then(({ sourceID }) => sourceID);
    }

    async lock(): Promise<void> {
        const isLocked = await this.isLocked();
        if (!isLocked) {
            await (await this.#getKeystoneSource()).lock();
            await this.onLocked();
        }
    }

    async isLocked(): Promise<boolean> {
        return (await this.#getKeystoneSource()).isLocked();
    }

    async passwordUnlock(password?: string): Promise<void> {
        const keystoneSource = await this.#getKeystoneSource();
        const isLocked = await keystoneSource.isLocked();
        if (isLocked) {
            if (!password) {
                throw new Error('Missing password to unlock the account');
            }

            await keystoneSource.unlock(password);
            await this.onUnlocked();
        }
    }

    async verifyPassword(password: string): Promise<void> {
        const keystoneSource = await this.#getKeystoneSource();
        await keystoneSource.verifyPassword(password);
    }

    async toUISerialized(): Promise<KeystoneAccountSerializedUI> {
        const { address, type, publicKey, derivationPath, selected, nickname, sourceID } =
            await this.getStoredData();
        const masterFingerprint = await (await this.#getKeystoneSource()).masterFingerprint;
        return {
            id: this.id,
            type,
            address,
            isLocked: await this.isLocked(),
            publicKey,
            derivationPath,
            lastUnlockedOn: await this.lastUnlockedOn,
            selected,
            nickname,
            isPasswordUnlockable: true,
            isKeyPairExportable: false,
            sourceID,
            masterFingerprint,
        };
    }

    async #getKeystoneSource() {
        return new KeystoneAccountSource((await this.getStoredData()).sourceID);
    }
}
