// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { FEATURES } from '_src/shared/experimentation/features';
import { useFeatureValue } from '@growthbook/growthbook-react';

const DEFAULT_REFETCH_INTERVAL = 20_000;
const DEFAULT_STALE_TIME = 20_000;

export function useCoinsReFetchingConfig() {
    const refetchInterval = useFeatureValue(
        FEATURES.WALLET_BALANCE_REFETCH_INTERVAL,
        DEFAULT_REFETCH_INTERVAL,
    );
    return {
        refetchInterval,
        staleTime: DEFAULT_STALE_TIME,
    };
}
