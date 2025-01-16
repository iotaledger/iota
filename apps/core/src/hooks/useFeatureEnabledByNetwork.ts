// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeature } from '@growthbook/growthbook-react';
import type { Network } from '@iota/iota-sdk/client';
import { Feature } from '../enums';

type NetworkBasedFeature = {
    [key in Network]: boolean;
};

export function useFeatureEnabledByNetwork(feature: Feature, network: Network): boolean {
    const featureFlag = useFeature<NetworkBasedFeature>(feature)?.value;
    return featureFlag?.[network] ?? false;
}
