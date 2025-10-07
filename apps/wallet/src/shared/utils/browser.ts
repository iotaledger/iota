// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Browser from 'webextension-polyfill';

export function openInNewTab(pathname?: string) {
    const baseUrl = Browser.runtime.getURL('ui.html');

    const query = new URLSearchParams({ type: 'tab' });

    const finalUrl = `${baseUrl}?${query.toString()}${pathname ? `#${pathname}` : ''}`;

    return Browser.tabs.create({ url: finalUrl });
}
