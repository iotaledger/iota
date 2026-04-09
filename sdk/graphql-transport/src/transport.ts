// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { TypedDocumentNode } from '@graphql-typed-document-node/core';
import type {
    IotaTransport,
    IotaTransportRequestOptions,
    IotaTransportSubscribeOptions,
    RequestInspector,
} from '@iota/iota-sdk/client';
import { IotaHTTPTransport } from '@iota/iota-sdk/client';
import type { DocumentNode } from 'graphql';
import { print } from 'graphql';

import {
    SubscribeEventsDocument,
    SubscribeTransactionsDocument,
    TypedDocumentString,
} from './generated/queries.js';
import type {
    SubscribeEventsSubscription,
    SubscribeEventsSubscriptionVariables,
    SubscribeTransactionsSubscription,
    SubscribeTransactionsSubscriptionVariables,
    SubscriptionEventFilter,
    SubscriptionTransactionFilter,
} from './generated/queries.js';
import type { GraphQLWebSocketClientOptions } from './graphql-websocket-client.js';
import { GraphQLWebSocketClient } from './graphql-websocket-client.js';
import { RPC_METHODS, UnsupportedMethodError, UnsupportedParamError } from './methods.js';
import { toShortTypeString } from './mappers/util.js';

export interface IotaClientGraphQLTransportOptions {
    url: string;
    wsUrl?: string;
    wsOptions?: Omit<GraphQLWebSocketClientOptions, 'WebSocketConstructor'>;
    WebSocketConstructor?: typeof WebSocket;
    fallbackTransportUrl?: string;
    fallbackMethods?: (keyof typeof RPC_METHODS)[];
    unsupportedMethods?: (keyof typeof RPC_METHODS)[];
    /** Optional inspector function for monitoring and tracing requests */
    inspector?: RequestInspector;
}

export type GraphQLDocument<
    Result = Record<string, unknown>,
    Variables = Record<string, unknown>,
> =
    | string
    | DocumentNode
    | TypedDocumentNode<Result, Variables>
    | TypedDocumentString<Result, Variables>;

export type GraphQLQueryOptions<
    Result = Record<string, unknown>,
    Variables = Record<string, unknown>,
> = {
    query: GraphQLDocument<Result, Variables>;
    operationName?: string;
    extensions?: Record<string, unknown>;
} & (Variables extends { [key: string]: never }
    ? { variables?: Variables }
    : {
          variables: Variables;
      });

export type GraphQLQueryResult<Result = Record<string, unknown>> = {
    data?: Result;
    errors?: GraphQLResponseErrors;
    extensions?: Record<string, unknown>;
};

export type GraphQLResponseErrors = Array<{
    message: string;
    locations?: { line: number; column: number }[];
    path?: (string | number)[];
}>;

export class IotaClientGraphQLTransport implements IotaTransport {
    #options: IotaClientGraphQLTransportOptions;
    #fallbackTransport?: IotaTransport;
    #fallbackMethods: (keyof typeof RPC_METHODS)[];
    #unsupportedMethods: (keyof typeof RPC_METHODS)[];
    #wsClient: GraphQLWebSocketClient | null = null;

    constructor(options: IotaClientGraphQLTransportOptions) {
        this.#options = options;
        this.#fallbackMethods = options.fallbackMethods || [
            'executeTransactionBlock',
            'dryRunTransactionBlock',
            'devInspectTransactionBlock',
            'getTotalTransactions',
            'getNetworkMetrics',
            'getParticipationMetrics',
            'getMoveCallMetrics',
            'getAllEpochAddressMetrics',
            'getEpochs',
            'getDynamicFieldObjectV2',
        ];
        this.#unsupportedMethods = options.unsupportedMethods || ['getOwnedObjects'];

        if (options.fallbackTransportUrl) {
            this.#fallbackTransport = new IotaHTTPTransport({
                url: options.fallbackTransportUrl,
                inspector: options.inspector,
            });
        }
    }

    #getWebSocketClient(): GraphQLWebSocketClient {
        if (!this.#wsClient) {
            const baseUrl = this.#options.wsUrl ?? this.#options.url;
            const endpoint = this.#options.wsUrl
                ? baseUrl
                : baseUrl.replace(/\/?$/, '/subscriptions');
            this.#wsClient = new GraphQLWebSocketClient(endpoint, {
                ...this.#options.wsOptions,
                ...(this.#options.WebSocketConstructor
                    ? { WebSocketConstructor: this.#options.WebSocketConstructor }
                    : {}),
            });
        }
        return this.#wsClient;
    }

    async graphqlQuery<
        Result = Record<string, unknown>,
        Variables = Record<string, unknown>,
        Data = Result,
    >(
        options: GraphQLQueryOptions<Result, Variables>,
        getData?: (result: Result) => Data,
    ): Promise<NonNullable<Data>> {
        const res = await this.graphqlRequest(options);

        if (!res.ok) {
            throw new Error('Failed to fetch');
        }

        const { data, errors } = (await res.json()) as GraphQLQueryResult<Result>;

        handleGraphQLErrors(errors);

        const extractedData = data && (getData ? getData(data) : data);

        if (extractedData == null) {
            throw new Error('Missing response data');
        }

        return extractedData as NonNullable<Data>;
    }

    async graphqlRequest<Result = Record<string, unknown>, Variables = Record<string, unknown>>(
        options: GraphQLQueryOptions<Result, Variables>,
    ): Promise<Response> {
        return fetch(this.#options.url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                query:
                    typeof options.query === 'string' ||
                    options.query instanceof TypedDocumentString
                        ? options.query.toString()
                        : print(options.query),
                variables: options.variables,
                extensions: options.extensions,
                operationName: options.operationName,
            }),
        });
    }

    async request<T = unknown>(input: IotaTransportRequestOptions): Promise<T> {
        const executeRequest = async () => {
            let clientMethod: keyof typeof RPC_METHODS;

            switch (input.method) {
                case 'rpc.discover':
                    clientMethod = 'getRpcApiVersion';
                    break;
                case 'iotax_getLatestAddressMetrics':
                    clientMethod = 'getAddressMetrics';
                    break;
                default:
                    clientMethod = input.method.split('_')[1] as keyof typeof RPC_METHODS;
            }

            // Methods with allowed fallback will go through GraphQL first and only default to JSON-RPC if they fail
            const allowFallback = this.#fallbackMethods.includes(clientMethod);
            // Unsupported methods will go through JSON-RPC directly
            const isUnsupported = this.#unsupportedMethods.includes(clientMethod);

            const method = RPC_METHODS[clientMethod];

            if (isUnsupported) {
                // If Unsupported we force to try fallback
                return await this.#tryUseFallback(input);
            }

            // No method and no fallback allowed
            if (!method && !allowFallback) {
                throw new UnsupportedMethodError(input.method);
            }

            try {
                // Method doesnt have a graphql implementation
                if (!method) throw new Error('Missing method');

                return await method(this, input.params as never);
            } catch (error) {
                // Method has an allowed fallback or is partially unsupported
                if (allowFallback || error instanceof UnsupportedParamError) {
                    return this.#tryUseFallback(input);
                } else {
                    throw error;
                }
            }
        };

        return this.#options.inspector
            ? this.#options.inspector(input, executeRequest as () => Promise<T>)
            : (executeRequest() as Promise<T>);
    }

    async subscribe<T = unknown>(
        input: IotaTransportSubscribeOptions<T>,
    ): Promise<() => Promise<boolean>> {
        switch (input.method) {
            case 'iotax_subscribeEvent':
                return this.#subscribeEvents(input);
            case 'iotax_subscribeTransaction':
                return this.#subscribeTransactions(input);
            default:
                break;
        }

        if (!this.#fallbackTransport) {
            throw new UnsupportedMethodError(input.method);
        }

        return this.#fallbackTransport.subscribe(input);
    }

    async #subscribeEvents<T>(
        input: IotaTransportSubscribeOptions<T>,
    ): Promise<() => Promise<boolean>> {
        const rpcFilter = input.params[0] as Record<string, unknown> | undefined;
        const variables: SubscribeEventsSubscriptionVariables = {
            filter: rpcFilter ? mapRpcEventFilterToGraphQL(rpcFilter) : undefined,
        };

        const client = this.#getWebSocketClient();
        return client.subscribe<SubscribeEventsSubscription>({
            query: SubscribeEventsDocument.toString(),
            variables: variables as Record<string, unknown>,
            onMessage: (data) => {
                const payload = data.events;
                if (payload.__typename === 'Lagged') {
                    return;
                }
                const event = payload as Exclude<typeof payload, { __typename?: 'Lagged' }>;
                const mapped = {
                    bcs: event.bcs,
                    id: {
                        eventSeq: '',
                        txDigest: '',
                    },
                    packageId: event.sendingModule?.package.address ?? '',
                    parsedJson: event.json,
                    sender: event.sender?.address,
                    timestampMs: event.timestamp
                        ? new Date(event.timestamp as string).getTime().toString()
                        : undefined,
                    transactionModule: event.sendingModule
                        ? `${event.sendingModule.package.address}::${event.sendingModule.name}`
                        : '',
                    type: toShortTypeString(event.type?.repr) ?? '',
                };
                input.onMessage(mapped as T);
            },
            onError: (errors) => {
                console.error('GraphQL subscription error (events):', errors);
            },
            signal: input.signal,
        });
    }

    async #subscribeTransactions<T>(
        input: IotaTransportSubscribeOptions<T>,
    ): Promise<() => Promise<boolean>> {
        const rpcFilter = input.params[0] as Record<string, unknown> | undefined;
        const variables: SubscribeTransactionsSubscriptionVariables = {
            filter: rpcFilter ? mapRpcTransactionFilterToGraphQL(rpcFilter) : undefined,
        };

        const client = this.#getWebSocketClient();
        return client.subscribe<SubscribeTransactionsSubscription>({
            query: SubscribeTransactionsDocument.toString(),
            variables: variables as Record<string, unknown>,
            onMessage: (data) => {
                const payload = data.transactions;
                if (payload.__typename === 'Lagged') {
                    return;
                }
                const tx = payload as Exclude<typeof payload, { __typename?: 'Lagged' }>;
                const mapped = {
                    bcs: tx.effects?.bcs,
                    digest: tx.digest,
                    ...(tx.effects?.timestamp
                        ? {
                              timestampMs: new Date(tx.effects.timestamp as string)
                                  .getTime()
                                  .toString(),
                          }
                        : {}),
                };
                input.onMessage(mapped as T);
            },
            onError: (errors) => {
                console.error('GraphQL subscription error (transactions):', errors);
            },
            signal: input.signal,
        });
    }

    async #tryUseFallback<T = unknown>(input: IotaTransportRequestOptions): Promise<T> {
        if (!this.#fallbackTransport) {
            throw new UnsupportedMethodError(input.method);
        }

        return this.#fallbackTransport.request(input);
    }
}

function handleGraphQLErrors(errors: GraphQLResponseErrors | undefined): void {
    if (!errors || errors.length === 0) return;

    const errorInstances = errors.map((error) => new GraphQLResponseError(error));

    if (errorInstances.length === 1) {
        throw errorInstances[0];
    }

    throw new AggregateError(errorInstances);
}

class GraphQLResponseError extends Error {
    locations?: Array<{ line: number; column: number }>;

    constructor(error: GraphQLResponseErrors[0]) {
        super(error.message);
        this.locations = error.locations;
    }
}

/**
 * Maps a JSON-RPC `IotaEventFilter` to the GraphQL `SubscriptionEventFilter`.
 *
 * The GraphQL subscription API only supports filtering by `emittingModule`,
 * which corresponds to the `Package` and `MoveModule` RPC filters.
 */
function mapRpcEventFilterToGraphQL(
    rpcFilter: Record<string, unknown>,
): SubscriptionEventFilter | undefined {
    if ('Package' in rpcFilter) {
        return { emittingModule: rpcFilter.Package as string };
    }

    if ('MoveModule' in rpcFilter) {
        const mod = rpcFilter.MoveModule as { package: string; module: string };
        return { emittingModule: `${mod.package}::${mod.module}` };
    }

    if ('MoveEventType' in rpcFilter) {
        const parts = (rpcFilter.MoveEventType as string).split('::');
        if (parts.length >= 2) {
            return { emittingModule: `${parts[0]}::${parts[1]}` };
        }
    }

    if ('MoveEventModule' in rpcFilter) {
        const mod = rpcFilter.MoveEventModule as { package: string; module: string };
        return { emittingModule: `${mod.package}::${mod.module}` };
    }

    return undefined;
}

function mapRpcTransactionFilterToGraphQL(
    rpcFilter: Record<string, unknown>,
): SubscriptionTransactionFilter | undefined {
    if ('TransactionKind' in rpcFilter) {
        return {
            kind: rpcFilter.TransactionKind as SubscriptionTransactionFilter & {
                kind: unknown;
            } extends { kind: infer K }
                ? K
                : never,
        } as SubscriptionTransactionFilter;
    }

    if ('FromAddress' in rpcFilter) {
        return {
            signingAddress: rpcFilter.FromAddress as string,
        } as SubscriptionTransactionFilter;
    }

    if ('MoveFunction' in rpcFilter) {
        const fn = rpcFilter.MoveFunction as {
            package: string;
            module?: string | null;
            function?: string | null;
        };
        let value = fn.package;
        if (fn.module) {
            value += `::${fn.module}`;
            if (fn.function) {
                value += `::${fn.function}`;
            }
        }
        return { function: value } as SubscriptionTransactionFilter;
    }

    return undefined;
}
