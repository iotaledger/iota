// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    BrowserPasskeyProvider,
    type BrowserPasswordProviderOptions,
} from '@iota/iota-sdk/keypairs/passkey';

const DEFAULT_PASSKEY_SAVED_NAME = 'iota-passkey-wallet';
const DEFAULT_ORIGIN = 'iota.org';

export const DEFAULT_PASSKEY_RP = {
    name: DEFAULT_PASSKEY_SAVED_NAME,
    id: DEFAULT_ORIGIN,
};

export const DEFAULT_AUTHENTICATOR_OPTIONS = {
    authenticatorAttachment: 'platform' as const,
};

/**
 * Creates browser password provider options with defaults applied
 */
export function createBrowserPasswordProviderOptions({
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
    providerOptions = {},
}: {
    providerOptions?: Partial<BrowserPasswordProviderOptions>;
} = {}): { provider: BrowserPasskeyProvider; options: BrowserPasswordProviderOptions } {
    const options = createBrowserPasswordProviderOptions({ options: providerOptions });
    const provider = new BrowserPasskeyProvider(
        options?.rp?.name ?? DEFAULT_PASSKEY_SAVED_NAME,
        options,
    );
    return {
        provider,
        options,
    };
}
