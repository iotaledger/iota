// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';
import { Network } from '@iota/iota.js/client';
import { IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS } from '@iota/iota.js/utils';

import useAppSelector from './useAppSelector';

const DEFAULT_RECOGNIZED_PACKAGES = [IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS];

export function useRecognizedPackages() {
    const network = useAppSelector((app) => app.app.network);
    const recognizedPackages = useFeatureValue('recognized-packages', DEFAULT_RECOGNIZED_PACKAGES);

    // Our recognized package list is currently only available on mainnet
    return network === Network.Mainnet ? recognizedPackages : DEFAULT_RECOGNIZED_PACKAGES;
}
