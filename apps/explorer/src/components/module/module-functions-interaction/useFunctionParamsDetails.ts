// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';

import { getNormalizedFunctionParameterTypeDetails } from '../utils';

import type { IotaMoveNormalizedType } from '@iota/iota.js/client';

export function useFunctionParamsDetails(
    params: IotaMoveNormalizedType[],
    functionTypeArgNames?: string[],
) {
    return useMemo(
        () =>
            params
                .map((aParam) =>
                    getNormalizedFunctionParameterTypeDetails(aParam, functionTypeArgNames),
                )
                .filter(({ isTxContext }) => !isTxContext),
        [params, functionTypeArgNames],
    );
}
