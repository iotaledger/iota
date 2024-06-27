// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { FEATURES } from '_src/shared/experimentation/features';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { COINS_QUERY_REFETCH_INTERVAL, COINS_QUERY_STALE_TIME } from '@iota/core';

export function useCoinsReFetchingConfig() {
    const refetchInterval = useFeatureValue(
        FEATURES.WALLET_BALANCE_REFETCH_INTERVAL,
        COINS_QUERY_REFETCH_INTERVAL,
    );
    return {
        refetchInterval,
        staleTime: COINS_QUERY_STALE_TIME,
    };
}
