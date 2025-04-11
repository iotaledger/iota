// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { filter, fromEventPattern, share, take, takeWhile } from 'rxjs';
import Browser from 'webextension-polyfill';

const POPUP_WIDTH = 360;
const POPUP_HEIGHT = 680;

export const windowRemovedStream = fromEventPattern<number>(
    (handler) => Browser.windows.onRemoved.addListener(handler),
    (handler) => Browser.windows.onRemoved.removeListener(handler),
).pipe(share());

export class Window {
    private _id: number | null = null;
    private _url: string;

    constructor(url: string) {
        this._url = url;
    }

    public async show() {
        const { width = 0, left = 0, top = 0 } = await Browser.windows.getLastFocused();
        const w = await Browser.windows.create({
            url: this._url,
            focused: true,
            width: POPUP_WIDTH,
            height: POPUP_HEIGHT,
            type: 'popup',
            top: top,
            left: Math.floor(left + width - 450),
        });
        this._id = typeof w.id === 'undefined' ? null : w.id;
        return windowRemovedStream.pipe(
            takeWhile(() => this._id !== null),
            filter((aWindowID) => aWindowID === this._id),
            take(1),
        );
    }

    public async close() {
        if (this._id !== null) {
            await Browser.windows.remove(this._id);
        }
    }

    /**
     * The id of the window.
     * {@link Window.show} has to be called first. Otherwise this will be null
     * */
    public get id(): number | null {
        return this._id;
    }
}
