// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AppsBackendClient } from '@iota/apps-backend-client';

// This is a separate client instance for the wallet UI, with flag values synced from the service worker.
export const appsBackendClient = new AppsBackendClient();
