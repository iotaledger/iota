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

import { TypedDocumentString } from './generated/queries.js';
import { RPC_METHODS, UnsupportedMethodError, UnsupportedParamError } from './methods.js';

export interface IotaClientGraphQLTransportOptions {
    url: string;
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
        if (!this.#fallbackTransport) {
            throw new UnsupportedMethodError(input.method);
        }

        return this.#fallbackTransport.subscribe(input);
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
