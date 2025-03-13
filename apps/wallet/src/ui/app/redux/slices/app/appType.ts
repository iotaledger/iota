// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Browser from 'webextension-polyfill';

export enum AppType {
    Unknown,
    Fullscreen,
    Popup,
}

export function getFromLocationSearch() {
    if (/type=popup/.test(window.location.search)) {
        return AppType.Popup;
    }
    return AppType.Fullscreen;
}

export function getIsAppViewPopup() {
    const views = Browser.extension.getViews({ type: 'popup' });
    return views.length !== 0;
}
