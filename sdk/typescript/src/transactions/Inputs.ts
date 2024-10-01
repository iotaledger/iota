// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toB64 } from '@iota/bcs';
import type { SerializedBcs } from '@iota/bcs';

import { normalizeIotaAddress } from '../utils/iota-types.js';
import type { CallArg, ObjectRef } from './data/internal.js';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function Pure(data: Uint8Array | SerializedBcs<any>): Extract<CallArg, { Pure: unknown }> {
    return {
        $kind: 'Pure',
        Pure: {
            bytes: data instanceof Uint8Array ? toB64(data) : data.toBase64(),
        },
    };
}

export const Inputs = {
    Pure,
    ObjectRef({ objectId, digest, version }: ObjectRef): Extract<CallArg, { Object: unknown }> {
        return {
            $kind: 'Object',
            Object: {
                $kind: 'ImmOrOwnedObject',
                ImmOrOwnedObject: {
                    digest,
                    version,
                    objectId: normalizeIotaAddress(objectId),
                },
            },
        };
    },
    SharedObjectRef({
        objectId,
        mutable,
        initialSharedVersion,
    }: {
        objectId: string;
        mutable: boolean;
        initialSharedVersion: number | string;
    }): Extract<CallArg, { Object: unknown }> {
        return {
            $kind: 'Object',
            Object: {
                $kind: 'SharedObject',
                SharedObject: {
                    mutable,
                    initialSharedVersion,
                    objectId: normalizeIotaAddress(objectId),
                },
            },
        };
    },
    ReceivingRef({ objectId, digest, version }: ObjectRef): Extract<CallArg, { Object: unknown }> {
        return {
            $kind: 'Object',
            Object: {
                $kind: 'Receiving',
                Receiving: {
                    digest,
                    version,
                    objectId: normalizeIotaAddress(objectId),
                },
            },
        };
    },
};
