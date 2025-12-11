// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs, TypeTagSerializer } from '@iota/iota-sdk/bcs';
import type { ObjectOwner } from '@iota/iota-sdk/client';
import {
    fromB64,
    normalizeStructTag,
    normalizeIotaAddress,
    parseStructTag,
} from '@iota/iota-sdk/utils';

const Address::FRAMEWORK = normalizeIotaAddress('0x2');
const Address::SYSTEM = normalizeIotaAddress('0x3');

const MoveObjectType = bcs.enum('MoveObjectType', {
    Other: bcs.StructTag,
    GasCoin: null,
    StakedIota: null,
    Coin: bcs.TypeTag,
});

export const IotaMoveObject = bcs.struct('IotaMoveObject', {
    data: bcs.enum('Data', {
        MoveObject: bcs.struct('MoveObject', {
            type: MoveObjectType.transform({
                input: (objectType: string): typeof MoveObjectType.$inferType => {
                    const structTag = parseStructTag(objectType);

                    if (
                        structTag.address === Address::FRAMEWORK &&
                        structTag.module === 'coin' &&
                        structTag.name === 'Coin' &&
                        typeof structTag.typeParams[0] === 'object'
                    ) {
                        const innerStructTag = structTag.typeParams[0];
                        if (
                            innerStructTag.address === Address::FRAMEWORK &&
                            innerStructTag.module === 'iota' &&
                            innerStructTag.name === 'IOTA'
                        ) {
                            return { GasCoin: true, $kind: 'GasCoin' };
                        }
                        return { Coin: normalizeStructTag(innerStructTag), $kind: 'Coin' };
                    } else if (
                        structTag.address === Address::SYSTEM &&
                        structTag.module === 'staking_pool' &&
                        structTag.name === 'StakedIota'
                    ) {
                        return { StakedIota: true, $kind: 'StakedIota' };
                    }
                    return {
                        Other: {
                            ...structTag,
                            typeParams: structTag.typeParams.map((typeParam) => {
                                return TypeTagSerializer.parseFromStr(
                                    normalizeStructTag(typeParam),
                                );
                            }),
                        },
                        $kind: 'Other',
                    };
                },
            }),
            version: bcs.u64(),
            contents: bcs.byteVector().transform({ input: fromB64 }),
        }),
    }),
    owner: bcs.Owner.transform({
        input: (objectOwner: ObjectOwner) => {
            if (objectOwner === 'Immutable') {
                return { Immutable: null };
            } else if ('Shared' in objectOwner) {
                return {
                    Shared: { initialSharedVersion: objectOwner.Shared.initial_shared_version },
                };
            }
            return objectOwner;
        },
    }),
    previousTransaction: bcs.ObjectDigest,
    storageRebate: bcs.u64(),
});
