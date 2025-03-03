// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaValidatorSummary } from '@iota/iota-sdk/client';

export type IotaValidatorSummaryExtended = IotaValidatorSummary & { isPending?: boolean };
