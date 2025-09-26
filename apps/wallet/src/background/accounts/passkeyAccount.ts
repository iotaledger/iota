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
import { type BrowserPasswordProviderOptions } from '@iota/iota-sdk/keypairs/passkey';

export interface PasskeyAccountSerialized extends SerializedAccount {
    type: AccountType.PasskeyDerived;
    encrypted: string;
    publicKey: string;
    providerOptions: BrowserPasswordProviderOptions;
}

export interface PasskeyAccountSerializedUI extends SerializedUIAccount {
    type: AccountType.PasskeyDerived;
    publicKey: string;
    providerOptions: BrowserPasswordProviderOptions;
}

export function isPasskeyAccountSerializedUI(
    account: SerializedUIAccount,
): account is PasskeyAccountSerializedUI {
    return account.type === AccountType.PasskeyDerived;
}

type EphemeralData = {
    unlocked: true;
};

export class PasskeyAccount
    extends Account<PasskeyAccountSerialized, EphemeralData>
    implements PasswordUnlockableAccount
{
    readonly unlockType = 'password';

    static async createNew(inputs: {
        password: string;
        address: string;
        publicKey: string;
        providerOptions: BrowserPasswordProviderOptions;
    }): Promise<Omit<PasskeyAccountSerialized, 'id'>> {
        return {
            type: AccountType.PasskeyDerived,
            address: inputs.address,
            publicKey: inputs.publicKey,
            providerOptions: inputs.providerOptions,
            encrypted: await encrypt(inputs.password, {}),
            lastUnlockedOn: null,
            selected: false,
            nickname: null,
            createdAt: Date.now(),
        };
    }

    static isOfType(serialized: SerializedAccount): serialized is PasskeyAccountSerialized {
        return serialized.type === AccountType.PasskeyDerived;
    }

    constructor({ id, cachedData }: { id: string; cachedData?: PasskeyAccountSerialized }) {
        super({ type: AccountType.PasskeyDerived, id, cachedData });
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

    async toUISerialized(): Promise<PasskeyAccountSerializedUI> {
        const { address, publicKey, type, selected, nickname, providerOptions } =
            await this.getStoredData();
        return {
            id: this.id,
            type,
            address,
            publicKey,
            isLocked: await this.isLocked(),
            lastUnlockedOn: await this.lastUnlockedOn,
            selected,
            nickname,
            isPasswordUnlockable: true,
            isKeyPairExportable: false,
            providerOptions,
        };
    }
}
