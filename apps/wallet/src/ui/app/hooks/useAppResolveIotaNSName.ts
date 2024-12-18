// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useFeatureIsOn } from '@growthbook/growthbook-react';
import { normalizeIotaNSName } from '@iota/iota-sdk/utils';

import { useResolveIotaNSName as useResolveIotaNSNameCore } from '../../../../../core';

export function useResolveIotaNSName(address?: string) {
    const enableNewIotaNSFormat = useFeatureIsOn('wallet-enable-new-iotans-name-format');
    const { data } = useResolveIotaNSNameCore(address);
    return data ? normalizeIotaNSName(data, enableNewIotaNSFormat ? 'at' : 'dot') : undefined;
}
