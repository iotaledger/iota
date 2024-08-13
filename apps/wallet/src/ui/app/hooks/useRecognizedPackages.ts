// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';
import { Network } from '@iota/iota-sdk/client';
import useAppSelector from './useAppSelector';
import { DEFAULT_RECOGNIZED_PACKAGES } from '@iota/core';

export function useRecognizedPackages() {
    const network = useAppSelector((app) => app.app.network);
    const recognizedPackages = useFeatureValue('recognized-packages', DEFAULT_RECOGNIZED_PACKAGES);

    // Our recognized package list is currently only available on mainnet
    return network === Network.Mainnet ? recognizedPackages : DEFAULT_RECOGNIZED_PACKAGES;
}
