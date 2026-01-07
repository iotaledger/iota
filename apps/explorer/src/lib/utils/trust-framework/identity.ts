// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as identity from '@iota/identity-wasm/web';
import { isValidIotaObjectId } from '@iota/iota-sdk/utils';

let initPromise: Promise<void> | null = null;

/**
 * Idempotent initialization of WASM module of Identity.
 *
 * Use it everytime you need to call any identity API.
 */
export const initIdentityWasmWeb = async (): Promise<void> => {
    if (!initPromise) {
        initPromise = identity.init().catch((e) => {
            console.error('failed to load identity wasm (web version)', e);
            initPromise = null; // allow retry
            throw e;
        });
    }
    return initPromise;
};

export async function tryDIDParse(didCandidate: string): Promise<identity.IotaDID | null> {
    try {
        await initIdentityWasmWeb();
        return identity.IotaDID.parse(didCandidate);
    } catch {
        return null;
    }
}

/**
 * Try generate an IotaDID from ObjectId and Network and return the generated did,
 * otherwise return null if not possible to generate by any reason.
 */
export async function tryGenerateDidFromObjectId(
    objectId: string,
    network: string,
): Promise<identity.IotaDID | null> {
    try {
        if (!isValidIotaObjectId(objectId)) return null;

        await initIdentityWasmWeb();
        return identity.IotaDID.fromAliasId(objectId, network);
    } catch {
        return null;
    }
}
