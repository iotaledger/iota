// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SerializedBcs } from '@iota/bcs';
import { isSerializedBcs } from '@iota/bcs';
import type { Infer } from 'superstruct';
import { array, bigint, boolean, integer, number, object, string, union } from 'superstruct';

import { bcs } from '../bcs/index.js';
import type { SharedObjectRef } from '../bcs/index.js';
import { normalizeIotaAddress } from '../utils/iota-types.js';

export const IotaObjectRef = object({
    /** Base64 string representing the object digest */
    digest: string(),
    /** Hex code as string representing the object id */
    objectId: string(),
    /** Object version */
    version: union([number(), string(), bigint()]),
});
export type IotaObjectRef = Infer<typeof IotaObjectRef>;

const ObjectArg = union([
    object({ ImmOrOwned: IotaObjectRef }),
    object({
        Shared: object({
            objectId: string(),
            initialSharedVersion: union([integer(), string()]),
            mutable: boolean(),
        }),
    }),
    object({ Receiving: IotaObjectRef }),
]);

export const PureCallArg = object({ Pure: array(integer()) });
export const ObjectCallArg = object({ Object: ObjectArg });
export type PureCallArg = Infer<typeof PureCallArg>;
export type ObjectCallArg = Infer<typeof ObjectCallArg>;

export const BuilderCallArg = union([PureCallArg, ObjectCallArg]);
export type BuilderCallArg = Infer<typeof BuilderCallArg>;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function Pure(data: Uint8Array | SerializedBcs<any>, type?: string): PureCallArg;
/** @deprecated pass SerializedBcs values instead */
function Pure(data: unknown, type?: string): PureCallArg;
function Pure(data: unknown, type?: string): PureCallArg {
    return {
        Pure: Array.from(
            data instanceof Uint8Array
                ? data
                : isSerializedBcs(data)
                  ? data.toBytes()
                  : // NOTE: We explicitly set this to be growable to infinity, because we have maxSize validation at the builder-level:
                    bcs.ser(type!, data, { maxSize: Infinity }).toBytes(),
        ),
    };
}

export const Inputs = {
    Pure,
    ObjectRef({ objectId, digest, version }: IotaObjectRef): ObjectCallArg {
        return {
            Object: {
                ImmOrOwned: {
                    digest,
                    version,
                    objectId: normalizeIotaAddress(objectId),
                },
            },
        };
    },
    SharedObjectRef({ objectId, mutable, initialSharedVersion }: SharedObjectRef): ObjectCallArg {
        return {
            Object: {
                Shared: {
                    mutable,
                    initialSharedVersion,
                    objectId: normalizeIotaAddress(objectId),
                },
            },
        };
    },
    ReceivingRef({ objectId, digest, version }: IotaObjectRef): ObjectCallArg {
        return {
            Object: {
                Receiving: {
                    digest,
                    version,
                    objectId: normalizeIotaAddress(objectId),
                },
            },
        };
    },
};

export function getIdFromCallArg(arg: string | ObjectCallArg) {
    if (typeof arg === 'string') {
        return normalizeIotaAddress(arg);
    }
    if ('ImmOrOwned' in arg.Object) {
        return normalizeIotaAddress(arg.Object.ImmOrOwned.objectId);
    }

    if ('Receiving' in arg.Object) {
        return normalizeIotaAddress(arg.Object.Receiving.objectId);
    }

    return normalizeIotaAddress(arg.Object.Shared.objectId);
}

export function getSharedObjectInput(arg: BuilderCallArg): SharedObjectRef | undefined {
    return typeof arg === 'object' && 'Object' in arg && 'Shared' in arg.Object
        ? arg.Object.Shared
        : undefined;
}

export function isSharedObjectInput(arg: BuilderCallArg): boolean {
    return !!getSharedObjectInput(arg);
}

export function isMutableSharedObjectInput(arg: BuilderCallArg): boolean {
    return getSharedObjectInput(arg)?.mutable ?? false;
}
