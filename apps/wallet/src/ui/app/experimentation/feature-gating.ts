// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';

// This is a separate growthbook instance for the wallet UI, with flag values synced from the service worker.
export const growthbook = new GrowthBook();
