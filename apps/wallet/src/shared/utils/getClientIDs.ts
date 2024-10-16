// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function getClientIDs(appName: string) {
    const id = appName.replace(/\s+/g, '-').toLowerCase();

    return {
        name: `${id}_in-page`,
        target: `${id}_content-script`,
    };
}
