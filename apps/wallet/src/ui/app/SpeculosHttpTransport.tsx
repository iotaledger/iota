// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import axios, { type AxiosInstance } from 'axios';
import { DisconnectedDevice } from '@ledgerhq/errors';
import Transport from '@ledgerhq/hw-transport';
import { log } from '@ledgerhq/logs';
import { Subject } from 'rxjs';

const SPECULOS_API_DEFAULT_PORT = '5000';

export type SpeculosHttpTransportOpts = {
    apiPort?: string;
    timeout?: number;
    baseURL?: string;
};

enum SpeculosButton {
    LEFT = 'Ll',
    RIGHT = 'Rr',
    BOTH = 'LRlr',
}

/**
 * Speculos TCP transport implementation
 *
 * @example
 * import SpeculosHttpTransport from "@ledgerhq/hw-transport-node-speculos-http";
 * const transport = await SpeculosHttpTransport.open();
 * const res = await transport.send(0xE0, 0x01, 0, 0);
 */
export default class SpeculosHttpTransport extends Transport {
    instance: AxiosInstance;
    opts: SpeculosHttpTransportOpts;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    eventStream: any; // ReadStream?
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    automationEvents: Subject<Record<string, any>> = new Subject();

    constructor(instance: AxiosInstance, opts: SpeculosHttpTransportOpts) {
        super();
        this.instance = instance;
        this.opts = opts;
    }

    static isSupported = (): Promise<boolean> => Promise.resolve(true);
    // this transport is not discoverable
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    static list = (): any => Promise.resolve([]);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    static listen = (_observer: any) => ({
        unsubscribe: () => {},
    });

    buttonTable = {
        [SpeculosButton.BOTH]: 'both',
        [SpeculosButton.RIGHT]: 'right',
        [SpeculosButton.LEFT]: 'left',
    };

    static open = (opts?: SpeculosHttpTransportOpts): Promise<SpeculosHttpTransport> =>
        new Promise((resolve, reject) => {
            const instance = axios.create({
                baseURL: `http://localhost:${opts?.apiPort || SPECULOS_API_DEFAULT_PORT}`,
                timeout: opts?.timeout,
                adapter: 'fetch', // Use fetch adapter for it to work in web env
            });

            const transport = new SpeculosHttpTransport(instance, opts ?? {});

            instance
                .get('/events?stream=true', {
                    headers: {
                        Accept: 'text/event-stream',
                    },
                    responseType: 'stream',
                })
                .then(async (response) => {
                    const stream = response.data;
                    const reader = stream.pipeThrough(new TextDecoderStream()).getReader();

                    const readChunk = async () => {
                        // eslint-disable-next-line no-constant-condition
                        while (true) {
                            const { value, done } = await reader.read();
                            if (done) {
                                break;
                            }

                            log('speculos-event', value);
                            const split = value.replace('data: ', '');
                            try {
                                const json = JSON.parse(split);
                                transport.automationEvents.next(json);
                            } catch (e) {
                                // @ts-expect-error any
                                log('speculos-event-error', e);
                            }
                        }

                        log('speculos-event', 'close');
                        transport.emit('disconnect', new DisconnectedDevice('Speculos exited!'));
                    };

                    reader
                        .read()
                        .then(readChunk)
                        // @ts-expect-error any
                        .catch((error) => {
                            log('speculos-event-error', error);
                            reject(error);
                        });

                    // Resolve the promise as soon as the stream is connected and handled
                    transport.eventStream = stream;
                    resolve(transport);
                })
                .catch((error) => {
                    reject(error);
                });
        });

    static check = async (opts?: SpeculosHttpTransportOpts): Promise<boolean> => {
        const instance = axios.create({
            baseURL: `http://localhost:${opts?.apiPort || SPECULOS_API_DEFAULT_PORT}`,
            timeout: opts?.timeout,
        });

        try {
            const response = await instance.get('/');
            if (response.status === 200) {
                return true;
            }
        } catch (err) {
            return false;
        }

        return false;
    };

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    async exchange(apdu: Buffer): Promise<any> {
        const hex = apdu.toString('hex');
        log('apdu', '=> ' + hex);
        return this.instance.post('/apdu', { data: hex }).then((r) => {
            // r.data is {"data": "hex value of response"}
            const data = r.data.data;
            log('apdu', '<= ' + data);
            return Buffer.from(data, 'hex');
        });
    }

    async close() {
        // close event stream
        if (!this.eventStream.locked) {
            this.eventStream.cancel();
        }
        return Promise.resolve();
    }
}
