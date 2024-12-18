// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';
import { Network } from '@iota/iota-sdk/client';
import { Feature, DEFAULT_RECOGNIZED_PACKAGES } from '../../';

export function useRecognizedPackages(network: Network): string[] {
    const recognizedPackages = useFeatureValue(
        Feature.RecognizedPackages,
        DEFAULT_RECOGNIZED_PACKAGES,
    );

    // Our recognized package list is currently only available on mainnet
    return network === Network.Mainnet ? recognizedPackages : DEFAULT_RECOGNIZED_PACKAGES;
}
