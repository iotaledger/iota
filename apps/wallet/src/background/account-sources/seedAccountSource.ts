// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { decrypt, encrypt } from '_src/shared/cryptography/keystore';
import { Ed25519Keypair, type Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { sha256 } from '@noble/hashes/sha256';
import { bytesToHex } from '@noble/hashes/utils';
import Dexie from 'dexie';

import { getAccountSources } from '.';
import { getAllAccounts } from '../accounts';
import { SeedAccount, type SeedSerializedAccount } from '../accounts/seedAccount';
import { setupAutoLockAlarm } from '../autoLockAccounts';
import { backupDB, getDB } from '../db';
import { makeUniqueKey } from '../storageUtils';
import {
    AccountSource,
    AccountSourceType,
    type AccountSourceSerialized,
    type AccountSourceSerializedUI,
} from './accountSource';
import { accountSourcesEvents } from './events';
import { type MakeDerivationOptions, makeDerivationPath } from './bip44Path';

type DataDecrypted = {
    seed: string;
};

interface SeedAccountSourceSerialized extends AccountSourceSerialized {
    type: AccountSourceType.Seed;
    encryptedData: string;
    // hash of entropy to be used for comparing sources (even when locked)
    sourceHash: string;
}

interface SeedAccountSourceSerializedUI extends AccountSourceSerializedUI {
    type: AccountSourceType.Seed;
}

function deriveKeypairFromSeed(seedHex: string, derivationPath: string) {
    return Ed25519Keypair.deriveKeypairFromSeed(seedHex, derivationPath);
}

export class SeedAccountSource extends AccountSource<SeedAccountSourceSerialized, DataDecrypted> {
    static async createNew({ password, seed }: { password: string; seed: string }) {
        const dataSerialized: SeedAccountSourceSerialized = {
            id: makeUniqueKey(),
            type: AccountSourceType.Seed,
            encryptedData: await SeedAccountSource.createEncryptedData(seed, password),
            sourceHash: bytesToHex(sha256(seed)),
            createdAt: Date.now(),
        };
        const allAccountSources = await getAccountSources();
        for (const anAccountSource of allAccountSources) {
            if (
                anAccountSource instanceof SeedAccountSource &&
                (await anAccountSource.sourceHash) === dataSerialized.sourceHash
            ) {
                throw new Error('Seed account source already exists');
            }
        }
        return dataSerialized;
    }

    static isOfType(
        serialized: AccountSourceSerialized,
    ): serialized is SeedAccountSourceSerialized {
        return serialized.type === AccountSourceType.Seed;
    }

    static async save(
        serialized: SeedAccountSourceSerialized,
        {
            skipBackup = false,
            skipEventEmit = false,
        }: { skipBackup?: boolean; skipEventEmit?: boolean } = {},
    ) {
        await (await Dexie.waitFor(getDB())).accountSources.put(serialized);
        if (!skipBackup) {
            await backupDB();
        }
        if (!skipEventEmit) {
            accountSourcesEvents.emit('accountSourcesChanged');
        }
        return new SeedAccountSource(serialized.id);
    }

    static createEncryptedData(seed: string, password: string) {
        const decryptedData: DataDecrypted = {
            seed,
        };
        return encrypt(password, decryptedData);
    }

    constructor(id: string) {
        super({ type: AccountSourceType.Seed, id });
    }

    async isLocked() {
        return (await this.getEphemeralValue()) === null;
    }

    async unlock(password: string) {
        await this.setEphemeralValue(await this.#decryptStoredData(password));
        await setupAutoLockAlarm();
        accountSourcesEvents.emit('accountSourceStatusUpdated', { accountSourceID: this.id });
    }

    async verifyPassword(password: string) {
        const { encryptedData } = await this.getStoredData();
        await decrypt<DataDecrypted>(password, encryptedData);
    }

    async lock() {
        await this.clearEphemeralValue();
        accountSourcesEvents.emit('accountSourceStatusUpdated', { accountSourceID: this.id });
    }

    async deriveAccount(
        derivationOptions?: MakeDerivationOptions,
    ): Promise<Omit<SeedSerializedAccount, 'id'>> {
        const derivationPath =
            typeof derivationOptions !== 'undefined'
                ? makeDerivationPath(derivationOptions)
                : await this.#getAvailableDerivationPath();
        const keyPair = await this.deriveKeyPair(derivationPath);
        return SeedAccount.createNew({ keyPair, derivationPath, sourceID: this.id });
    }

    async derivePubKey(derivationOptions: MakeDerivationOptions): Promise<Ed25519PublicKey> {
        const keyPair = await this.deriveKeyPair(makeDerivationPath(derivationOptions));
        return keyPair.getPublicKey();
    }

    async deriveKeyPair(derivationPath: string) {
        const data = await this.getEphemeralValue();
        if (!data) {
            throw new Error(`Seed account source ${this.id} is locked`);
        }
        return deriveKeypairFromSeed(data.seed, derivationPath);
    }

    async toUISerialized(): Promise<SeedAccountSourceSerializedUI> {
        const { type } = await this.getStoredData();
        return {
            id: this.id,
            type,
            isLocked: await this.isLocked(),
        };
    }

    async getSeed(password?: string) {
        let data = await this.getEphemeralValue();
        if (password && !data) {
            data = await this.#decryptStoredData(password);
        }
        if (!data) {
            throw new Error(`Seed account source ${this.id} is locked`);
        }
        return data.seed;
    }

    get sourceHash() {
        return this.getStoredData().then(({ sourceHash }) => sourceHash);
    }

    async verifyRecoveryData(seed: string) {
        const newSeedHash = bytesToHex(sha256(seed));
        if (newSeedHash !== (await this.sourceHash)) {
            throw new Error("Wrong seed, doesn't match the existing one");
        }
        return true;
    }

    async #getAvailableDerivationPath() {
        const derivationPathMap: Record<string, boolean> = {};
        for (const anAccount of await getAllAccounts({ sourceID: this.id })) {
            if (anAccount instanceof SeedAccount && (await anAccount.sourceID) === this.id) {
                derivationPathMap[await anAccount.derivationPath] = true;
            }
        }
        let index = 0;
        let derivationPath = '';
        let temp;
        do {
            temp = makeDerivationPath({
                accountIndex: index++,
            });
            if (!derivationPathMap[temp]) {
                derivationPath = temp;
            }
        } while (derivationPath === '' && index < 10000);
        if (!derivationPath) {
            throw new Error('Failed to find next available derivation path');
        }
        return derivationPath;
    }

    async #decryptStoredData(password: string) {
        const { encryptedData } = await this.getStoredData();
        return decrypt<DataDecrypted>(password, encryptedData);
    }
}
