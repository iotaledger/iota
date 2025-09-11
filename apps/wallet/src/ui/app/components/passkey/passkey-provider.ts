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
    providerOptions = {},
}: {
    providerOptions?: Partial<BrowserPasswordProviderOptions>;
} = {}): BrowserPasswordProviderOptions {
    // Extract values from providerOptions or use defaults
    const rpName = providerOptions.rp?.name || DEFAULT_PASSKEY_SAVED_NAME;
    const rpId = providerOptions.rp?.id || DEFAULT_PASSKEY_RP.id;
    const authenticatorAttachment =
        providerOptions.authenticatorSelection?.authenticatorAttachment ||
        DEFAULT_AUTHENTICATOR_OPTIONS.authenticatorAttachment;

    // Create options with defaults for any missing values
    return {
        ...providerOptions,
        rp: {
            name: rpName,
            id: rpId,
            ...(providerOptions.rp || {}),
        },
        authenticatorSelection: {
            authenticatorAttachment,
            ...(providerOptions.authenticatorSelection || {}),
        },
    };
}

export function createBrowserPasskeyProvider({
    providerOptions = {},
}: {
    providerOptions?: Partial<BrowserPasswordProviderOptions>;
} = {}): BrowserPasskeyProvider {
    const options = createBrowserPasskeyProviderOptions({ providerOptions });
    return new BrowserPasskeyProvider(options?.rp?.name ?? DEFAULT_PASSKEY_SAVED_NAME, options);
}
