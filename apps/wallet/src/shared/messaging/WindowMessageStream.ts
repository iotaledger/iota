// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Message } from '_messages';
import { filter, fromEvent, map, share } from 'rxjs';
import type { Observable } from 'rxjs';

export type ClientType = string;

interface ClientConnection {
    name: ClientType;
    target: ClientType;
}

type WindowMessage = {
    target: ClientType;
    payload: Message;
};

export class WindowMessageStream {
    public readonly messages: Observable<Message>;
    private _name: ClientType;
    private _target: ClientType;

    constructor(name: ClientType, target: ClientType) {
        if (name === target) {
            throw new Error('[WindowMessageStream] name and target must be different');
        }
        this._name = name;
        this._target = target;
        this.messages = fromEvent<MessageEvent<WindowMessage>>(window, 'message').pipe(
            filter((message) => message.source === window && message.data.target === this._name),
            map((message) => message.data.payload),
            share(),
        );
    }

    public send(payload: Message) {
        const msg: WindowMessage = {
            target: this._target,
            payload,
        };
        window.postMessage(msg);
    }

    private static cleanAppName(appName: string): string {
        return appName.replace(/\s+/g, '-').toLowerCase();
    }

    public static getClientIDs(appName: string = 'iota'): ClientConnection {
        const id = this.cleanAppName(appName);

        return {
            name: `${id}_in-page`,
            target: `${id}_content-script`,
        };
    }
}
