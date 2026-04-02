// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export { AppsBackendClient } from './client';
export type {
    AppsBackendClientOptions,
    CoinPriceResponse,
    FeatureDefinition,
    FeaturesResponse,
    FeatureResult,
    MonitorNetworkResponse,
    ProductAnalyticsConfigResponse,
} from './types';
export {
    AppsBackendClientProvider,
    useAppsBackendClient,
    useFeature,
    useFeatureValue,
    useFeatureIsOn,
} from './react';
