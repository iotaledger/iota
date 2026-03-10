// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeature } from '@growthbook/growthbook-react';
import type { Network } from '@iota/iota-sdk/client';

import { Feature } from '../lib/enums';

type NetworkBasedFeature = {
    [id in Network]: boolean;
};

export function useFeatureEnabledByNetwork(feature: Feature, network: Network): boolean {
    const featureFlag = useFeature<NetworkBasedFeature>(feature)?.value;
    return featureFlag?.[network] ?? false;
}
