// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Browser from 'webextension-polyfill';

export enum ExtensionViewType {
    Unknown = 'unknown',
    Popup = 'popup',
    FullScreen = 'fullScreen',
    SidePanel = 'sidePanel',
}

export function getAppViewType(): ExtensionViewType {
    const currentView = window;
    if (Browser.extension.getViews({ type: 'tab' }).includes(currentView)) {
        return ExtensionViewType.FullScreen;
    }
    if (Browser.extension.getViews({ type: 'popup' }).includes(currentView)) {
        return ExtensionViewType.Popup;
    }
    if (Browser.extension.getViews().includes(currentView)) {
        return ExtensionViewType.SidePanel;
    }
    return ExtensionViewType.Popup;
}
