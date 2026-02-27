// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isSerializedBcs } from '@iota/bcs';
import type { SerializedBcs } from '@iota/bcs';
import { bcs } from '../bcs/index.js';
import { pureBcsSchemaFromTypeName } from '../bcs/pure.js';
import type { PureTypeName, ShapeFromPureTypeName, ValidPureTypeName } from '../bcs/pure.js';

export function createPure<T>(makePure: (value: SerializedBcs<any, any> | Uint8Array) => T) {
    function pure<T extends PureTypeName>(
        type: T extends PureTypeName ? ValidPureTypeName<T> : T,
        value: ShapeFromPureTypeName<T>,
    ): T;

    function pure(
        /**
         * The pure value, serialized to BCS. If this is a Uint8Array, then the value
         * is assumed to be raw bytes, and will be used directly.
         */
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        value: SerializedBcs<any, any> | Uint8Array,
    ): T;

    function pure(
        typeOrSerializedValue?: PureTypeName | SerializedBcs<any, any> | Uint8Array,
        value?: unknown,
    ): T {
        if (typeof typeOrSerializedValue === 'string') {
            return makePure(
                pureBcsSchemaFromTypeName(typeOrSerializedValue).serialize(value as never),
            );
        }

        if (typeOrSerializedValue instanceof Uint8Array || isSerializedBcs(typeOrSerializedValue)) {
            return makePure(typeOrSerializedValue);
        }

        throw new Error('tx.pure must be called either a bcs type name, or a serialized bcs value');
    }

    pure.u8 = (value: number) => makePure(bcs.U8.serialize(value));
    pure.u16 = (value: number) => makePure(bcs.U16.serialize(value));
    pure.u32 = (value: number) => makePure(bcs.U32.serialize(value));
    pure.u64 = (value: bigint | number | string) => makePure(bcs.U64.serialize(value));
    pure.u128 = (value: bigint | number | string) => makePure(bcs.U128.serialize(value));
    pure.u256 = (value: bigint | number | string) => makePure(bcs.U256.serialize(value));
    pure.bool = (value: boolean) => makePure(bcs.Bool.serialize(value));
    pure.string = (value: string) => makePure(bcs.String.serialize(value));
    pure.address = (value: string) => makePure(bcs.Address.serialize(value));
    pure.id = pure.address;
    pure.vector = <Type extends PureTypeName>(
        type: T extends PureTypeName ? ValidPureTypeName<Type> : Type,
        value: Iterable<ShapeFromPureTypeName<Type>> & { length: number },
    ) => {
        return makePure(
            bcs.vector(pureBcsSchemaFromTypeName(type as PureTypeName)).serialize(value as never),
        );
    };
    pure.option = <Type extends PureTypeName>(
        type: T extends PureTypeName ? ValidPureTypeName<Type> : Type,
        value: ShapeFromPureTypeName<Type> | null | undefined,
    ) => {
        return makePure(bcs.option(pureBcsSchemaFromTypeName(type)).serialize(value as never));
    };

    return pure;
}
