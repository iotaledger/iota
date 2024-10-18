// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Feature } from '@iota/core';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS } from '@iota/iota-sdk/utils';
import { Network } from '@iota/iota-sdk/client';

import { useNetwork } from './';

const DEFAULT_RECOGNIZED_PACKAGES = [IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS];

export function useRecognizedPackages(): string[] {
    const [network] = useNetwork();

    const recognizedPackages = useFeatureValue(
        Feature.RecognizedPackages,
        DEFAULT_RECOGNIZED_PACKAGES,
    );

    // Our recognized package list is currently only available on mainnet
    return network === Network.Mainnet ? recognizedPackages : DEFAULT_RECOGNIZED_PACKAGES;
}
