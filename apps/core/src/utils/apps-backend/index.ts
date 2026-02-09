// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function getAppsBackend(): string {
    return process.env.APPS_BACKEND as string;
}
