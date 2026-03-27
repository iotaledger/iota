// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const DEFAULT_APPS_BACKEND = 'http://localhost:3003';

export function getAppsBackend(): string {
    return process.env.APPS_BACKEND || DEFAULT_APPS_BACKEND;
}
