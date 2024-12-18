// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { API_ENV } from '_src/shared/api-env';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS } from '@iota/iota-sdk/utils';

import useAppSelector from './useAppSelector';

const DEFAULT_RECOGNIZED_PACKAGES = [IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS];

export function useRecognizedPackages() {
	const apiEnv = useAppSelector((app) => app.app.apiEnv);
	const recognizedPackages = useFeatureValue('recognized-packages', DEFAULT_RECOGNIZED_PACKAGES);

	// Our recognized package list is currently only available on mainnet
	return apiEnv === API_ENV.mainnet ? recognizedPackages : DEFAULT_RECOGNIZED_PACKAGES;
}
