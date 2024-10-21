// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DEFAULT_APP_NAME } from '../constants';

export function generateWalletMessageStreamIdentifiers(appName: string = DEFAULT_APP_NAME) {
    const id = appName.replace(/\s+/g, '-').toLowerCase();

    return {
        name: `${id}_in-page`,
        target: `${id}_content-script`,
    };
}
