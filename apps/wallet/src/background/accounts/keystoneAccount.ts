// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { decrypt, encrypt } from '_src/shared/cryptography/keystore';

import {
    Account,
    AccountType,
    type PasswordUnlockableAccount,
    type SerializedAccount,
    type SerializedUIAccount,
} from './account';

export interface KeystoneAccountSerialized extends SerializedAccount {
    type: AccountType.KeystoneDerived;
    derivationPath: string;
    // just used for authentication nothing is stored here at the moment
    encrypted: string;
    masterFingerprint: string;
}

export interface KeystoneAccountSerializedUI extends SerializedUIAccount {
    type: AccountType.KeystoneDerived;
    derivationPath: string;
    masterFingerprint: string;
}

export function isKeystoneAccountSerializedUI(
    account: SerializedUIAccount,
): account is KeystoneAccountSerializedUI {
    return account.type === AccountType.KeystoneDerived;
}

type EphemeralData = {
    unlocked: true;
};

export class KeystoneAccount
    extends Account<KeystoneAccountSerialized, EphemeralData>
    implements PasswordUnlockableAccount
{
    readonly unlockType = 'password';

    static async createNew({
        address,
        publicKey,
        password,
        derivationPath,
        masterFingerprint,
    }: {
        address: string;
        publicKey: string | null;
        password: string;
        derivationPath: string;
        masterFingerprint: string;
    }): Promise<Omit<KeystoneAccountSerialized, 'id'>> {
        return {
            type: AccountType.KeystoneDerived,
            address,
            publicKey,
            encrypted: await encrypt(password, {}),
            derivationPath,
            lastUnlockedOn: null,
            selected: false,
            nickname: null,
            createdAt: Date.now(),
            masterFingerprint,
        };
    }

    static isOfType(serialized: SerializedAccount): serialized is KeystoneAccountSerialized {
        return serialized.type === AccountType.KeystoneDerived;
    }

    constructor({ id, cachedData }: { id: string; cachedData?: KeystoneAccountSerialized }) {
        super({ type: AccountType.KeystoneDerived, id, cachedData });
    }

    async lock(allowRead = false): Promise<void> {
        await this.clearEphemeralValue();
        await this.onLocked(allowRead);
    }

    async isLocked(): Promise<boolean> {
        return !(await this.getEphemeralValue())?.unlocked;
    }

    async passwordUnlock(password?: string): Promise<void> {
        if (!password) {
            throw new Error('Missing password to unlock the account');
        }
        const { encrypted } = await this.getStoredData();
        await decrypt<string>(password, encrypted);
        await this.setEphemeralValue({ unlocked: true });
        await this.onUnlocked();
    }

    async verifyPassword(password: string): Promise<void> {
        const { encrypted } = await this.getStoredData();
        await decrypt<string>(password, encrypted);
    }

    async toUISerialized(): Promise<KeystoneAccountSerializedUI> {
        const { address, type, publicKey, derivationPath, selected, nickname, masterFingerprint } =
            await this.getStoredData();
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
            masterFingerprint,
        };
    }
}
