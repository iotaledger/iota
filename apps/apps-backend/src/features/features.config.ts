// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface FeaturesConfig {
    defaultValue: boolean;
}

export enum Feature {
    AccountFinder = 'account-finder',
}

export const featuresDevelopment: Record<Feature, FeaturesConfig> = {
    [Feature.AccountFinder]: {
        defaultValue: false,
    },
};
