// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromExportedKeypair } from '_src/shared/utils';
import { type Keypair } from '@iota/iota-sdk/cryptography';

import { SeedAccountSource } from '../account-sources/seedAccountSource';
import {
    Account,
    AccountType,
    type KeyPairExportableAccount,
    type PasswordUnlockableAccount,
    type SerializedAccount,
    type SerializedUIAccount,
    type SigningAccount,
} from './account';

export interface SeedSerializedAccount extends SerializedAccount {
    type: AccountType.SeedDerived;
    sourceID: string;
    derivationPath: string;
    publicKey: string;
}

export interface SeedSerializedUiAccount extends SerializedUIAccount {
    type: AccountType.SeedDerived;
    publicKey: string;
    derivationPath: string;
    sourceID: string;
}

export function isSeedSerializedUiAccount(
    account: SerializedUIAccount,
): account is SeedSerializedUiAccount {
    return account.type === AccountType.SeedDerived;
}

type SessionStorageData = { keyPair: string };

export class SeedAccount
    extends Account<SeedSerializedAccount, SessionStorageData>
    implements PasswordUnlockableAccount, SigningAccount, KeyPairExportableAccount
{
    readonly unlockType = 'password' as const;
    readonly canSign = true;
    readonly exportableKeyPair = true;

    static isOfType(serialized: SerializedAccount): serialized is SeedSerializedAccount {
        return serialized.type === AccountType.SeedDerived;
    }

    static createNew({
        keyPair,
        derivationPath,
        sourceID,
    }: {
        keyPair: Keypair;
        derivationPath: string;
        sourceID: string;
    }): Omit<SeedSerializedAccount, 'id'> {
        return {
            type: AccountType.SeedDerived,
            sourceID,
            address: keyPair.getPublicKey().toIotaAddress(),
            derivationPath,
            publicKey: keyPair.getPublicKey().toBase64(),
            lastUnlockedOn: null,
            selected: false,
            nickname: null,
            createdAt: Date.now(),
        };
    }

    constructor({ id, cachedData }: { id: string; cachedData?: SeedSerializedAccount }) {
        super({ type: AccountType.SeedDerived, id, cachedData });
    }

    async isLocked(): Promise<boolean> {
        return !(await this.#getKeyPair());
    }

    async lock(allowRead = false): Promise<void> {
        await this.clearEphemeralValue();
        await this.onLocked(allowRead);
    }

    async passwordUnlock(password?: string): Promise<void> {
        const seedSource = await this.#getSeedSource();
        if ((await seedSource.isLocked()) && !password) {
            throw new Error('Missing password to unlock the account');
        }
        const { derivationPath } = await this.getStoredData();
        if (password) {
            await seedSource.unlock(password);
        }
        await this.setEphemeralValue({
            keyPair: (await seedSource.deriveKeyPair(derivationPath)).getSecretKey(),
        });
        await this.onUnlocked();
    }

    async verifyPassword(password: string): Promise<void> {
        const seedSource = await this.#getSeedSource();
        await seedSource.verifyPassword(password);
    }

    async toUISerialized(): Promise<SeedSerializedUiAccount> {
        const { id, type, address, derivationPath, publicKey, sourceID, selected, nickname } =
            await this.getStoredData();
        return {
            id,
            type,
            address,
            isLocked: await this.isLocked(),
            derivationPath,
            publicKey,
            sourceID,
            lastUnlockedOn: await this.lastUnlockedOn,
            selected,
            nickname,
            isPasswordUnlockable: true,
            isKeyPairExportable: true,
        };
    }

    async signData(data: Uint8Array): Promise<string> {
        const keyPair = await this.#getKeyPair();
        if (!keyPair) {
            throw new Error(`Account is locked`);
        }
        return this.generateSignature(data, keyPair);
    }

    get derivationPath() {
        return this.getCachedData().then(({ derivationPath }) => derivationPath);
    }

    get sourceID() {
        return this.getCachedData().then(({ sourceID }) => sourceID);
    }

    async exportKeyPair(password: string): Promise<string> {
        const { derivationPath } = await this.getStoredData();
        const seedSource = await this.#getSeedSource();
        await seedSource.unlock(password);
        return (await seedSource.deriveKeyPair(derivationPath)).getSecretKey();
    }

    async #getKeyPair() {
        const ephemeralData = await this.getEphemeralValue();
        if (ephemeralData) {
            return fromExportedKeypair(ephemeralData.keyPair);
        }
        return null;
    }

    async #getSeedSource() {
        return new SeedAccountSource((await this.getStoredData()).sourceID);
    }
}
