// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';

import { getNormalizedFunctionParameterTypeDetails } from '~/lib/utils';

import type { IotaMoveNormalizedType } from '@iota/iota-sdk/client';

interface FunctionParamsDetails {
    params: IotaMoveNormalizedType[];
    functionTypeArgNames?: string[];
}

export function useFunctionParamsDetails({ params, functionTypeArgNames }: FunctionParamsDetails) {
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
