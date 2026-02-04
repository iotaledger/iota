// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The webextension polyfill does not support (neither plans to) the chrome `sidePanel` API, so instead we do our little abstraction here

export class SidePanel {
    private static _isOpen: boolean = false;

    static isSupported(): boolean {
        return 'chrome' in window;
    }

    static isOpen(): boolean {
        return this._isOpen;
    }

    static _setOpen(open: boolean): void {
        this._isOpen = open;
    }

    static async isEnabled() {
        const options = await chrome.sidePanel.getOptions({});
        return options.enabled || false;
    }

    static async enableAndGoTo(path: string) {
        await chrome.sidePanel.setPanelBehavior({
            openPanelOnActionClick: true,
        });
        await chrome.sidePanel.setOptions({
            path,
            enabled: true,
        });
    }

    static async disable() {
        await chrome.sidePanel.setPanelBehavior({
            openPanelOnActionClick: false,
        });
        await chrome.sidePanel.setOptions({
            enabled: false,
        });
    }

    static async open(path: string) {
        const window = await chrome.windows.getCurrent({ populate: true });

        if (!window.id) {
            throw new Error('Failed to detect Window');
        }

        await this.enableAndGoTo(path);

        await chrome.sidePanel.open({ windowId: window.id });
    }

    static async close() {
        const window = await chrome.windows.getCurrent({ populate: true });

        if (!window.id) {
            throw new Error('Failed to detect Window');
        }

        // @ts-expect-error `close` does indeed exist, not sure why its not included in the types
        await chrome.sidePanel.close({ windowId: window.id });
    }
}
