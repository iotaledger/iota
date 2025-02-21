// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { decrypt, encrypt } from '_src/shared/cryptography/keystore';
import { fromExportedKeypair } from '_src/shared/utils';

import {
    Account,
    AccountType,
    type KeyPairExportableAccount,
    type PasswordUnlockableAccount,
    type SerializedAccount,
    type SerializedUIAccount,
    type SigningAccount,
} from './account';

type SessionStorageData = { keyPair: string };
type EncryptedData = { keyPair: string };

export interface ImportedAccountSerialized extends SerializedAccount {
    type: AccountType.PrivateKeyDerived;
    encrypted: string;
    publicKey: string;
}

export interface ImportedAccountSerializedUI extends SerializedUIAccount {
    type: AccountType.PrivateKeyDerived;
    publicKey: string;
}

export function isImportedAccountSerializedUI(
    account: SerializedUIAccount,
): account is ImportedAccountSerializedUI {
    return account.type === AccountType.PrivateKeyDerived;
}

export class ImportedAccount
    extends Account<ImportedAccountSerialized, SessionStorageData>
    implements PasswordUnlockableAccount, SigningAccount, KeyPairExportableAccount
{
    readonly canSign = true;
    readonly unlockType = 'password' as const;
    readonly exportableKeyPair = true;

    static async createNew(inputs: {
        keyPair: string;
        password: string;
    }): Promise<Omit<ImportedAccountSerialized, 'id'>> {
        const keyPair = fromExportedKeypair(inputs.keyPair);
        const dataToEncrypt: EncryptedData = {
            keyPair: inputs.keyPair,
        };
        return {
            type: AccountType.PrivateKeyDerived,
            address: keyPair.getPublicKey().toIotaAddress(),
            publicKey: keyPair.getPublicKey().toBase64(),
            encrypted: await encrypt(inputs.password, dataToEncrypt),
            lastUnlockedOn: null,
            selected: false,
            nickname: null,
            createdAt: Date.now(),
        };
    }

    static isOfType(serialized: SerializedAccount): serialized is ImportedAccountSerialized {
        return serialized.type === AccountType.PrivateKeyDerived;
    }

    constructor({ id, cachedData }: { id: string; cachedData?: ImportedAccountSerialized }) {
        super({ type: AccountType.PrivateKeyDerived, id, cachedData });
    }

    async lock(allowRead = false): Promise<void> {
        await this.clearEphemeralValue();
        await this.onLocked(allowRead);
    }

    async isLocked(): Promise<boolean> {
        return !(await this.#getKeyPair());
    }

    async toUISerialized(): Promise<ImportedAccountSerializedUI> {
        const { address, publicKey, type, selected, nickname } = await this.getStoredData();
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
            isKeyPairExportable: true,
        };
    }

    async passwordUnlock(password?: string): Promise<void> {
        if (!password) {
            throw new Error('Missing password to unlock the account');
        }
        const { encrypted } = await this.getStoredData();
        const { keyPair } = await decrypt<EncryptedData>(password, encrypted);
        await this.setEphemeralValue({ keyPair });
        await this.onUnlocked();
    }

    async verifyPassword(password: string): Promise<void> {
        const { encrypted } = await this.getStoredData();
        await decrypt<EncryptedData>(password, encrypted);
    }

    async signData(data: Uint8Array): Promise<string> {
        const keyPair = await this.#getKeyPair();
        if (!keyPair) {
            throw new Error(`Account is locked`);
        }
        return this.generateSignature(data, keyPair);
    }

    async exportKeyPair(password: string): Promise<string> {
        const { encrypted } = await this.getStoredData();
        const { keyPair } = await decrypt<EncryptedData>(password, encrypted);
        return fromExportedKeypair(keyPair).getSecretKey();
    }

    async #getKeyPair() {
        const ephemeralData = await this.getEphemeralValue();
        if (ephemeralData) {
            return fromExportedKeypair(ephemeralData.keyPair);
        }
        return null;
    }
}
