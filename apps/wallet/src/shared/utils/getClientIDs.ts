// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function getClientIDs(appName?: string) {
    const cleanAppName = (appName: string) => appName.replace(/\s+/g, '-').toLowerCase();
    const id = appName ? cleanAppName(appName) : 'iota';

    return {
        name: `${id}_in-page`,
        target: `${id}_content-script`,
    };
}
