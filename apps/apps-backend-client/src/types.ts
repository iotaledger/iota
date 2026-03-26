// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface AppsBackendClientOptions {
    url: string;
}

export interface FeatureDefinition<T = unknown> {
    defaultValue?: T;
}

export interface FeaturesResponse {
    status: number;
    features: Record<string, FeatureDefinition>;
    dateUpdated: string;
}

export interface FeatureResult<T> {
    value: T | null;
    on: boolean;
    off: boolean;
    source: string;
}
