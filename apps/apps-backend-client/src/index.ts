// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export { AppsBackendClient } from './client';
export type {
    AppsBackendClientOptions,
    FeatureDefinition,
    FeaturesResponse,
    FeatureResult,
} from './types';
export {
    AppsBackendClientProvider,
    useAppsBackendClient,
    useFeature,
    useFeatureValue,
    useFeatureIsOn,
} from './react';
