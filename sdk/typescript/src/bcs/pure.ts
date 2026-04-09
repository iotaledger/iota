// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/bcs';
import type { BcsType } from '@iota/bcs';

import { Address } from './bcs.js';

export type BasePureType =
    | 'u8'
    | 'u16'
    | 'u32'
    | 'u64'
    | 'u128'
    | 'u256'
    | 'bool'
    | 'id'
    | 'string'
    | 'address';

interface PureShapeByType {
    u8: number;
    u16: number;
    u32: number;
    u64: bigint | string | number;
    u128: bigint | string | number;
    u256: bigint | string | number;
    bool: boolean;
    string: string;
    id: string | Uint8Array;
    address: string | Uint8Array;
}

export type PureTypeName = BasePureType | `vector<${string}>` | `option<${string}>`;
export type ValidPureTypeName<T extends string> = T extends BasePureType
    ? PureTypeName
    : T extends `vector<${infer U}>`
      ? ValidPureTypeName<U>
      : T extends `option<${infer U}>`
        ? ValidPureTypeName<U>
        : PureTypeValidationError<T>;

export type ShapeFromPureTypeName<T extends PureTypeName> = T extends BasePureType
    ? PureShapeByType[T]
    : T extends `vector<${infer U extends PureTypeName}>`
      ? ShapeFromPureTypeName<U>[]
      : T extends `option<${infer U extends PureTypeName}>`
        ? ShapeFromPureTypeName<U> | null
        : never;

type PureTypeValidationError<T extends string> = {
    error: `Invalid Pure type name: ${T}`;
    // eslint-disable-next-line @typescript-eslint/ban-types
} & {};

export function pureBcsSchemaFromTypeName<T extends PureTypeName>(
    name: T extends PureTypeName ? ValidPureTypeName<T> : T,
): BcsType<ShapeFromPureTypeName<T>> {
    switch (name) {
        case 'u8':
            return bcs.u8() as never;
        case 'u16':
            return bcs.u16() as never;
        case 'u32':
            return bcs.u32() as never;
        case 'u64':
            return bcs.u64() as never;
        case 'u128':
            return bcs.u128() as never;
        case 'u256':
            return bcs.u256() as never;
        case 'bool':
            return bcs.bool() as never;
        case 'string':
            return bcs.string() as never;
        case 'id':
        case 'address':
            return Address as never;
    }

    const generic = (name as string).match(/^(vector|option)<(.+)>$/);
    if (generic) {
        const [kind, inner] = generic.slice(1);
        if (kind === 'vector') {
            return bcs.vector(pureBcsSchemaFromTypeName(inner as PureTypeName)) as never;
        } else {
            return bcs.option(pureBcsSchemaFromTypeName(inner as PureTypeName)) as never;
        }
    }

    throw new Error(`Invalid Pure type name: ${name}`);
}
