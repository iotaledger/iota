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

export enum ExtensionViewType {
    Popup = 'popup',
    Tab = 'tab',
    SidePanel = 'sidePanel',
}
export function getAppViewType(): ExtensionViewType {
    const currentView = window;
    if (Browser.extension.getViews({ type: 'tab' }).includes(currentView)) {
        return ExtensionViewType.Tab;
    }
    if (Browser.extension.getViews({ type: 'popup' }).includes(currentView)) {
        return ExtensionViewType.Popup;
    }
    if (Browser.extension.getViews().includes(currentView)) {
        return ExtensionViewType.SidePanel;
    }
    return ExtensionViewType.Popup;
}
