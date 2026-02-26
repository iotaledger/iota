// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { GraphQLDocument } from './client.js';

interface ConnectionInitMessage {
    type: 'connection_init';
    payload?: Record<string, unknown>;
}

interface ConnectionAckMessage {
    type: 'connection_ack';
    payload?: Record<string, unknown>;
}

interface SubscribeMessage {
    id: string;
    type: 'subscribe';
    payload: {
        query: string;
        variables?: Record<string, unknown>;
        operationName?: string;
        extensions?: Record<string, unknown>;
    };
}

interface NextMessage {
    id: string;
    type: 'next';
    payload: {
        data?: unknown;
        errors?: Array<{ message: string }>;
    };
}

interface ErrorMessage {
    id: string;
    type: 'error';
    payload: Array<{ message: string }>;
}

interface CompleteMessage {
    id: string;
    type: 'complete';
}

interface PingMessage {
    type: 'ping';
    payload?: Record<string, unknown>;
}

interface PongMessage {
    type: 'pong';
    payload?: Record<string, unknown>;
}

type ServerMessage =
    | ConnectionAckMessage
    | NextMessage
    | ErrorMessage
    | CompleteMessage
    | PingMessage;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type GraphQLSubscriptionRequest<T = any> = {
    query: GraphQLDocument;
    variables?: Record<string, unknown>;
    onMessage: (data: T) => void;
    onError?: (errors: Array<{ message: string }>) => void;
    onComplete?: () => void;
    signal?: AbortSignal;
};

/**
 * Configuration options for the GraphQL WebSocket client.
 */
export type GraphQLWebSocketClientOptions = {
    /**
     * Custom WebSocket class to use.
     * Defaults to the global WebSocket class, if available.
     */
    WebSocketConstructor?: typeof WebSocket;
    /**
     * Optional payload to send with the `connection_init` message.
     * Can be used for authentication tokens or other connection-level metadata.
     */
    connectionParams?: Record<string, unknown> | undefined;
    /**
     * Milliseconds to wait for the `connection_ack` message before timing out.
     * @default 30000
     */
    connectionAckTimeout?: number;
    /**
     * Milliseconds between reconnection attempts.
     * @default 3000
     */
    reconnectTimeout?: number;
    /**
     * Maximum number of reconnection attempts before giving up.
     * @default 5
     */
    maxReconnects?: number;
};

type ResolvedGraphQLWebSocketClientOptions = {
    WebSocketConstructor: typeof WebSocket;
    connectionParams: Record<string, unknown> | undefined;
    connectionAckTimeout: number;
    reconnectTimeout: number;
    maxReconnects: number;
};

const DEFAULT_OPTIONS: ResolvedGraphQLWebSocketClientOptions = {
    WebSocketConstructor: (typeof WebSocket !== 'undefined'
        ? WebSocket
        : undefined) as typeof WebSocket,
    connectionParams: undefined,
    connectionAckTimeout: 30_000,
    reconnectTimeout: 3_000,
    maxReconnects: 5,
};

function toWebSocketUrl(httpUrl: string): string {
    const url = new URL(httpUrl);
    url.protocol = url.protocol.replace('http', 'ws');
    return url.toString();
}

export class GraphQLWebSocketClient {
    #endpoint: string;
    #options: ResolvedGraphQLWebSocketClientOptions;
    #subscriptionId = 0;
    #disconnects = 0;
    #webSocket: WebSocket | null = null;
    #connectionPromise: Promise<WebSocket> | null = null;
    #subscriptions = new Map<string, GraphQLSubscription>();

    constructor(endpoint: string, options: GraphQLWebSocketClientOptions = {}) {
        this.#options = { ...DEFAULT_OPTIONS, ...options };

        if (!this.#options.WebSocketConstructor) {
            throw new Error(
                'Missing WebSocket constructor. Provide a WebSocketConstructor option or ensure the global WebSocket is available.',
            );
        }

        this.#endpoint = endpoint.startsWith('ws') ? endpoint : toWebSocketUrl(endpoint);
    }

    async subscribe<T>(request: GraphQLSubscriptionRequest<T>): Promise<() => Promise<boolean>> {
        const subscription = new GraphQLSubscription(request);
        const id = this.#nextId();
        this.#subscriptions.set(id, subscription);

        await subscription.subscribe(this, id);

        // Handle AbortSignal
        if (request.signal) {
            request.signal.addEventListener('abort', () => {
                subscription.unsubscribe(this, id);
                this.#subscriptions.delete(id);
            });
        }

        return async () => {
            const result = await subscription.unsubscribe(this, id);
            this.#subscriptions.delete(id);
            return result;
        };
    }

    async send(
        message: ConnectionInitMessage | SubscribeMessage | CompleteMessage | PongMessage,
    ): Promise<void> {
        const ws = await this.#setupWebSocket();
        ws.send(JSON.stringify(message));
    }

    close(): void {
        this.#subscriptions.clear();
        this.#webSocket?.close();
        this.#webSocket = null;
        this.#connectionPromise = null;
    }

    #nextId(): string {
        this.#subscriptionId += 1;
        return String(this.#subscriptionId);
    }

    #setupWebSocket(): Promise<WebSocket> {
        if (this.#connectionPromise) {
            return this.#connectionPromise;
        }

        this.#connectionPromise = new Promise<WebSocket>((resolve, reject) => {
            const ws = new this.#options.WebSocketConstructor(
                this.#endpoint,
                'graphql-transport-ws',
            );
            this.#webSocket = ws;

            // Timeout for the connection_ack
            const ackTimeout = setTimeout(() => {
                ws.close();
                reject(new Error('Connection acknowledgement timeout'));
            }, this.#options.connectionAckTimeout);

            let acknowledged = false;

            ws.addEventListener('open', () => {
                const initMessage: ConnectionInitMessage = {
                    type: 'connection_init',
                    ...(this.#options.connectionParams
                        ? { payload: this.#options.connectionParams }
                        : {}),
                };
                ws.send(JSON.stringify(initMessage));
            });

            ws.addEventListener('message', ({ data }: { data: string }) => {
                let message: ServerMessage;
                try {
                    message = JSON.parse(data) as ServerMessage;
                } catch (error) {
                    console.error(
                        new Error(`Failed to parse GraphQL WebSocket message: ${data}`, {
                            cause: error,
                        }),
                    );
                    return;
                }

                switch (message.type) {
                    case 'connection_ack':
                        clearTimeout(ackTimeout);
                        acknowledged = true;
                        this.#disconnects = 0;
                        resolve(ws);
                        break;

                    case 'next':
                        this.#subscriptions.get(message.id)?.onMessage(message.payload);
                        break;

                    case 'error':
                        this.#subscriptions.get(message.id)?.onError(message.payload);
                        break;

                    case 'complete':
                        this.#subscriptions.get(message.id)?.onComplete();
                        this.#subscriptions.delete(message.id);
                        break;

                    case 'ping':
                        // Respond to server keep-alive
                        ws.send(JSON.stringify({ type: 'pong' } satisfies PongMessage));
                        break;
                }
            });

            ws.addEventListener('close', () => {
                if (!acknowledged) {
                    clearTimeout(ackTimeout);
                    reject(new Error('WebSocket closed before connection was acknowledged'));
                    return;
                }

                this.#connectionPromise = null;
                this.#disconnects++;

                if (this.#disconnects <= this.#options.maxReconnects) {
                    setTimeout(() => {
                        this.#reconnect();
                    }, this.#options.reconnectTimeout);
                }
            });

            ws.addEventListener('error', () => {
                if (!acknowledged) {
                    clearTimeout(ackTimeout);
                    reject(new Error('WebSocket connection error'));
                }
            });
        });

        return this.#connectionPromise;
    }

    async #reconnect(): Promise<void> {
        this.#webSocket?.close();
        this.#connectionPromise = null;

        const entries = [...this.#subscriptions.entries()];
        await Promise.allSettled(
            entries.map(([id, subscription]) => subscription.subscribe(this, id)),
        );
    }
}

class GraphQLSubscription {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    #request: GraphQLSubscriptionRequest<any>;
    #active = false;

    constructor(request: GraphQLSubscriptionRequest) {
        this.#request = request;
    }

    async subscribe(client: GraphQLWebSocketClient, id: string): Promise<void> {
        this.#active = true;
        await client.send({
            id,
            type: 'subscribe',
            payload: {
                query: this.#request.query as string,
                ...(this.#request.variables ? { variables: this.#request.variables } : {}),
            },
        });
    }

    async unsubscribe(client: GraphQLWebSocketClient, id: string): Promise<boolean> {
        if (!this.#active) return false;
        this.#active = false;
        try {
            await client.send({ id, type: 'complete' });
            return true;
        } catch {
            return false;
        }
    }

    onMessage(payload: NextMessage['payload']): void {
        if (!this.#active) return;

        if (payload.errors && payload.errors.length > 0) {
            this.#request.onError?.(payload.errors);
            return;
        }

        if (payload.data != null) {
            this.#request.onMessage(payload.data);
        }
    }

    onError(errors: ErrorMessage['payload']): void {
        this.#active = false;
        this.#request.onError?.(errors);
    }

    onComplete(): void {
        this.#active = false;
        this.#request.onComplete?.();
    }
}
