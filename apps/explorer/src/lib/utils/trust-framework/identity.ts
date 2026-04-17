// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as identity from '@iota/identity-wasm/web';
import { isValidIotaObjectId } from '@iota/iota-sdk/utils';
import { type IotaClient, Network } from '@iota/iota-sdk/client';
import {
    DID_PROTOCOL_SEGMENT_SYMBOL,
    DID_URL_SEGMENT_SYMBOL,
    IDENTITY_WASM_PATH,
    IOTA_IDENTITY_PKG_ID,
} from '~/lib/constants/trustFramework.constants';

const regularNetworks = new Set([Network.Mainnet, Network.Testnet, Network.Devnet]);
let initPromise: Promise<void> | null = null;

/**
 * Idempotent initialization of WASM module of Identity.
 *
 * Use it everytime you need to call any identity API.
 */
export const initIdentityWasmWeb = async (): Promise<void> => {
    if (!initPromise) {
        initPromise = identity.init(IDENTITY_WASM_PATH).catch((e) => {
            console.error('failed to load identity wasm (web version)', e);
            initPromise = null; // allow retry
            throw e;
        });
    }
    return initPromise;
};

export const createIdentityClientReadOnly = async (
    iotaClient: IotaClient,
    network: string,
): Promise<identity.IdentityClientReadOnly> => {
    // If IOTA_IDENTITY_PKG_ID is declared it has precedence
    await initIdentityWasmWeb();
    if (IOTA_IDENTITY_PKG_ID != null) {
        return await identity.IdentityClientReadOnly.createWithPkgId(
            iotaClient,
            IOTA_IDENTITY_PKG_ID,
        );
    }

    // Well-known networks have well-known identity package id
    if (regularNetworks.has(network as Network)) {
        return await identity.IdentityClientReadOnly.create(iotaClient);
    }

    throw new Error(
        'Failed to create an IdentityClientReadOnly; declare IOTA_IDENTITY_PKG_ID environment if running on a custom network.',
    );
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

/**
 * Encode a DID to be represented in URL by replacing ':' for '-' returning a string,
 * otherwise returning null;
 * @example
 *    await tryEncodeDidToUrl(identity.IotaDID.parse('did:iota:ef77060e:0x06ed4ae8eb655e5063cc3c949c60fd7306a1612390b5ed350b32fec22e118943'))
 *    // output: did-iota-ef77060e-0x06ed4ae8eb655e5063cc3c949c60fd7306a1612390b5ed350b32fec22e118943
 */
export async function tryEncodeDidToUrl(did: identity.IotaDID | string): Promise<string | null> {
    try {
        await initIdentityWasmWeb();
        const isStr = typeof did === 'string';
        const didStr = isStr ? did : did.toString();
        const encodedDid = didStr.replaceAll(DID_PROTOCOL_SEGMENT_SYMBOL, DID_URL_SEGMENT_SYMBOL);
        return encodedDid;
    } catch {
        return null;
    }
}

/**
 * Decode a URL representation of a DID by replacing '-' for ':' returning a DID object,
 * otherwise returning null;
 * @example
 *    (await tryDecodeDidFromUrl('did-iota-ef77060e-0x06ed4ae8eb655e5063cc3c949c60fd7306a1612390b5ed350b32fec22e118943')).toString()
 *    // output: 'did:iota:ef77060e:0x06ed4ae8eb655e5063cc3c949c60fd7306a1612390b5ed350b32fec22e118943'
 */
export async function tryDecodeDidFromUrl(encodedDid: string): Promise<identity.IotaDID | null> {
    try {
        await initIdentityWasmWeb();
        const didStr = encodedDid.replaceAll(DID_URL_SEGMENT_SYMBOL, DID_PROTOCOL_SEGMENT_SYMBOL);
        const did = identity.IotaDID.parse(didStr);
        return did;
    } catch {
        return null;
    }
}
