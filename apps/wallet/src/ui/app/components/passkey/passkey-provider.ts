// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BrowserPasskeyProvider } from '@iota/iota-sdk/keypairs/passkey';

export const PASSKEY_SAVED_NAME = 'iota-passkey-wallet';

export const PASSKEY_PROVIDER = new BrowserPasskeyProvider(PASSKEY_SAVED_NAME, {
    rp: {
        name: PASSKEY_SAVED_NAME,
        id: window.location.hostname,
    },
    // authenticatorSelection: {
    //     authenticatorAttachment: 'cross-platform',
    //     residentKey: 'required',
    //     userVerification: 'required',
    // },
});
