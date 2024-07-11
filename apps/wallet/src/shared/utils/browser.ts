// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Browser from 'webextension-polyfill';

export const MAIN_UI_URL = Browser.runtime.getURL('ui.html');

export function openInNewTab() {
    return Browser.tabs.create({ url: MAIN_UI_URL });
}
