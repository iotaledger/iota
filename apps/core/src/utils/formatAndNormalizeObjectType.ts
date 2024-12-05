// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    formatAddress,
    formatType,
    normalizeStructTag,
    parseStructTag,
} from '@iota/iota-sdk/utils';

export function formatAndNormalizeObjectType(objectType: string): {
    address: string;
    structTag: string;
} {
    const { address, module, typeParams, ...rest } = parseStructTag(objectType);

    const formattedTypeParams = typeParams.map((typeParam) => {
        if (typeof typeParam === 'string') {
            return typeParam;
        } else {
            return {
                ...typeParam,
                address: formatAddress(typeParam.address),
            };
        }
    });

    const structTag = {
        address: formatAddress(address),
        module,
        typeParams: formattedTypeParams,
        ...rest,
    };

    const normalizedStructTag = formatType(normalizeStructTag(structTag));
    return { address, structTag: normalizedStructTag };
}
