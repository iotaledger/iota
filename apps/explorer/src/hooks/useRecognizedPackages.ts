// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';
import { DEFAULT_RECOGNIZED_PACKAGES, Feature } from '@iota/core';
import { Network } from '@iota/iota-sdk/client';
import { useNetwork } from './useNetwork';

export function useRecognizedPackages(): string[] {
    const [network] = useNetwork();
    const recognizedPackages = useFeatureValue(
        Feature.RecognizedPackages,
        DEFAULT_RECOGNIZED_PACKAGES,
    );

    // Our recognized package list is currently only available on mainnet
    return network === Network.Mainnet ? recognizedPackages : DEFAULT_RECOGNIZED_PACKAGES;
}
