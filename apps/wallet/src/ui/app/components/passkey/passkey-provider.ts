// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    BrowserPasskeyProvider,
    type BrowserPasswordProviderOptions,
} from '@iota/iota-sdk/keypairs/passkey';

export const DEFAULT_PASSKEY_SAVED_NAME = 'iota-passkey-wallet';

export const DEFAULT_PASSKEY_RP = {
    name: DEFAULT_PASSKEY_SAVED_NAME,
    id: window.location.hostname,
};

export const DEFAULT_AUTHENTICATOR_OPTIONS = {
    authenticatorAttachment: 'platform' as const,
};

/**
 * Creates browser passkey provider options with defaults applied
 */
export function createBrowserPasskeyProviderOptions({
    options = {},
}: {
    options?: Partial<BrowserPasswordProviderOptions>;
} = {}): BrowserPasswordProviderOptions {
    const providerOptions = {
        ...options,
        rp: {
            name: DEFAULT_PASSKEY_RP.name,
            id: DEFAULT_PASSKEY_RP.id,
            ...options?.rp,
        },
        authenticatorSelection: {
            authenticatorAttachment: DEFAULT_AUTHENTICATOR_OPTIONS.authenticatorAttachment,
            ...options?.authenticatorSelection,
        },
    };
    if (options?.user) {
        providerOptions.user = { ...options.user };
    }
    return providerOptions;
}

export function createBrowserPasskeyProvider({
    options = {},
}: {
    options?: Partial<BrowserPasswordProviderOptions>;
} = {}): BrowserPasskeyProvider {
    const providerOptions = createBrowserPasskeyProviderOptions({ options });
    return new BrowserPasskeyProvider(DEFAULT_PASSKEY_SAVED_NAME, providerOptions);
}
