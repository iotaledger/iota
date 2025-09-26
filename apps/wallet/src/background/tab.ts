// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { filter, fromEventPattern, share, take, takeWhile } from 'rxjs';
import Browser from 'webextension-polyfill';

export const tabRemovedStream = fromEventPattern<number>(
    (handler) => Browser.tabs.onRemoved.addListener((e) => handler(e)),
    (handler) => Browser.tabs.onRemoved.removeListener((e) => handler(e)),
).pipe(share());

export class Tab {
    private _id: number | null = null;
    private _url: string;
    private _openerTabId: number | null = null;
    private _insertIndex: number | null = null;

    constructor(url: string) {
        this._url = url;
    }

    public async show() {
        const requestingTab = (
            await Browser.tabs
                .query({ active: true, currentWindow: true })
                .catch(() => [{ index: 0, id: null }])
        )[0];

        this._openerTabId = typeof requestingTab.id === 'number' ? requestingTab.id : null;
        this._insertIndex = typeof requestingTab.index === 'number' ? requestingTab.index : null;

        const tab = await Browser.tabs
            .create({
                url: this._url,
                index: this._insertIndex != null ? this._insertIndex + 1 : undefined,
                active: true,
            })
            .catch((e) => {
                // eslint-disable-next-line no-console
                console.error('Failed to create a new tab:', e);
                return Browser.tabs.create({ url: this._url, active: true });
            });

        this._id = typeof tab.id === 'undefined' ? null : tab.id;
        return tabRemovedStream.pipe(
            takeWhile(() => this._id !== null),
            filter((aTabId) => aTabId === this._id),
            take(1),
        );
    }

    public async close() {
        if (this._id === null) return;
        try {
            const openerTabId = this._openerTabId;
            if (typeof openerTabId === 'number') {
                await Browser.tabs.update(openerTabId, { active: true }).catch(() => {});
            }
            await Browser.tabs.remove(this._id);
        } catch {
            // tab could be already closed, ignore errors
        } finally {
            this._id = null;
            this._openerTabId = null;
            this._insertIndex = null;
        }
    }

    /**
     * The id of the tab.
     * {@link Tab.show} has to be called first. Otherwise this will be null
     * */
    public get id(): number | null {
        return this._id;
    }
}
