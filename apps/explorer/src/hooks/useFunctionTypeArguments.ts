// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';

import type { IotaMoveAbilitySet } from '@iota/iota-sdk/client';

export function useFunctionTypeArguments(typeArguments: IotaMoveAbilitySet[]) {
    return useMemo(
        () =>
            typeArguments.map(
                (aTypeArgument, index) =>
                    `T${index}${
                        aTypeArgument.abilities.length
                            ? `: ${aTypeArgument.abilities.join(' + ')}`
                            : ''
                    }`,
            ),
        [typeArguments],
    );
}
