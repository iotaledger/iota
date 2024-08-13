// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { type IotaArgument } from '@iota/iota-sdk/client';

export function flattenIotaArguments(data: (IotaArgument | IotaArgument[])[]): string {
    if (!data) {
        return '';
    }

    return data
        .map((value) => {
            if (value === 'GasCoin') {
                return value;
            } else if (Array.isArray(value)) {
                return `[${flattenIotaArguments(value)}]`;
            } else if (value === null) {
                return 'Null';
            } else if (typeof value === 'object') {
                if ('Input' in value) {
                    return `Input(${value.Input})`;
                } else if ('Result' in value) {
                    return `Result(${value.Result})`;
                } else if ('NestedResult' in value) {
                    return `NestedResult(${value.NestedResult[0]}, ${value.NestedResult[1]})`;
                }
            } else {
                throw new Error('Not a correct flattenable data');
            }
            return '';
        })
        .join(', ');
}
