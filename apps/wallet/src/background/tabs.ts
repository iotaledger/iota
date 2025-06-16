// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BehaviorSubject, filter, fromEventPattern, merge, share, Subject } from 'rxjs';
import Browser from 'webextension-polyfill';
import type { Tabs as BrowserTabs } from 'webextension-polyfill';
// With activeTab, we can still track URL changes in the active tab
const onActiveTabUpdated = fromEventPattern<
    [number, BrowserTabs.OnUpdatedChangeInfoType, BrowserTabs.Tab]
>(
    (handler) => Browser.tabs.onUpdated.addListener(handler),
    (handler) => Browser.tabs.onUpdated.removeListener(handler),
).pipe(
    // Filter to include only active tabs
    filter(([_, __, tab]) => tab.active === true),
    share(),
);

const onTabActivated = fromEventPattern<BrowserTabs.OnActivatedActiveInfoType>(
    (handler) => Browser.tabs.onActivated.addListener(handler),
    (handler) => Browser.tabs.onActivated.removeListener(handler),
).pipe(share());

const onWindowFocusChanged = fromEventPattern<number>(
    (handler) => Browser.windows.onFocusChanged.addListener(handler),
    (handler) => Browser.windows.onFocusChanged.removeListener(handler),
).pipe(share());

type TabInfo = {
    id: number;
    url: string | null;
    nextUrl?: string;
    closed?: boolean;
};

type ActiveOriginInfo = {
    origin: string | null;
    favIcon: string | null;
};

class Tabs {
    private activeTab: TabInfo | null = null;
    private _onRemoved: Subject<TabInfo> = new Subject();
    private _onActiveOrigin: BehaviorSubject<ActiveOriginInfo> =
        new BehaviorSubject<ActiveOriginInfo>({ origin: null, favIcon: null });

    constructor() {
        // Initialize with current active tab
        this.refreshActiveTab();

        // Track active tab changes
        merge(
            onTabActivated,
            onWindowFocusChanged.pipe(
                // Only track when a window is focused (not when unfocused)
                filter((windowId) => windowId !== Browser.windows.WINDOW_ID_NONE),
            ),
        ).subscribe(() => {
            this.refreshActiveTab();
        });

        // Track URL changes in the active tab
        onActiveTabUpdated
            .pipe(
                // Only process URL changes
                filter(([_, changeInfo]) => !!changeInfo.url),
            )
            .subscribe(([tabId, changeInfo, tab]) => {
                if (
                    this.activeTab &&
                    this.activeTab.id === tabId &&
                    this.activeTab.url !== changeInfo.url
                ) {
                    // Create tab info for the URL change event
                    const tabInfo: TabInfo = {
                        id: tabId,
                        url: this.activeTab.url,
                        nextUrl: changeInfo.url,
                        closed: false,
                    };

                    // Update active tab
                    this.activeTab = {
                        id: tabId,
                        url: changeInfo.url || null,
                    };

                    // Emit removed event for URL change
                    this._onRemoved.next(tabInfo);

                    // Update origin
                    if (changeInfo.url) {
                        try {
                            const origin = new URL(changeInfo.url).origin;
                            this._onActiveOrigin.next({
                                origin,
                                favIcon: tab.favIconUrl || null,
                            });
                        } catch (e) {
                            // Invalid URL, ignore
                        }
                    }
                }
            });
    }

    /**
     * Refreshes the active tab information
     * Call this when you need up-to-date info about the active tab
     */
    public async refreshActiveTab(): Promise<void> {
        const tabs = await Browser.tabs.query({ active: true, currentWindow: true });
        if (tabs.length > 0) {
            const tab = tabs[0];
            const tabId = tab.id;
            const url = tab.url || null;

            if (tabId) {
                // Store active tab info
                this.activeTab = { id: tabId, url };

                // Update origin info
                if (url) {
                    try {
                        const origin = new URL(url).origin;
                        this._onActiveOrigin.next({
                            origin,
                            favIcon: tab.favIconUrl || null,
                        });
                    } catch (e) {
                        // Invalid URL
                        this._onActiveOrigin.next({
                            origin: null,
                            favIcon: null,
                        });
                    }
                }
            }
        }
    }

    /**
     * An observable that emits when a tab's URL has changed or when permission-related tabs close
     * With activeTab permission, this is limited to the active tab
     */
    public get onRemoved() {
        return this._onRemoved.asObservable();
    }

    /**
     * Observable for tracking the active origin
     */
    public get activeOrigin() {
        return this._onActiveOrigin.asObservable();
    }
}

const tabs = new Tabs();
export default tabs;
