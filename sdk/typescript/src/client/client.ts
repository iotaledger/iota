// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { fromB58, toB64, toHEX } from '@iota/bcs';

import type { Signer } from '../cryptography/index.js';
import type { TransactionBlock } from '../transactions/index.js';
import { isTransactionBlock } from '../transactions/index.js';
import {
    isValidIotaAddress,
    isValidIotaObjectId,
    isValidTransactionDigest,
    normalizeIotaAddress,
    normalizeIotaObjectId,
} from '../utils/iota-types.js';
import { IotaHTTPTransport } from './http-transport.js';
import type { IotaTransport } from './http-transport.js';
import type {
    AddressMetrics,
    AllEpochsAddressMetrics,
    Checkpoint,
    CheckpointPage,
    CoinBalance,
    CoinMetadata,
    CoinSupply,
    CommitteeInfo,
    DelegatedStake,
    DevInspectResults,
    DevInspectTransactionBlockParams,
    DryRunTransactionBlockParams,
    DryRunTransactionBlockResponse,
    DynamicFieldPage,
    EpochInfo,
    EpochMetricsPage,
    EpochPage,
    ExecuteTransactionBlockParams,
    GetAllBalancesParams,
    GetAllCoinsParams,
    GetBalanceParams,
    GetCheckpointParams,
    GetCheckpointsParams,
    GetCoinMetadataParams,
    GetCoinsParams,
    GetCommitteeInfoParams,
    GetDynamicFieldObjectParams,
    GetDynamicFieldsParams,
    GetMoveFunctionArgTypesParams,
    GetNormalizedMoveFunctionParams,
    GetNormalizedMoveModuleParams,
    GetNormalizedMoveModulesByPackageParams,
    GetNormalizedMoveStructParams,
    GetObjectParams,
    GetOwnedObjectsParams,
    GetProtocolConfigParams,
    GetStakesByIdsParams,
    GetStakesParams,
    GetTotalSupplyParams,
    GetTransactionBlockParams,
    MoveCallMetrics,
    MultiGetObjectsParams,
    MultiGetTransactionBlocksParams,
    NetworkMetrics,
    ObjectRead,
    Order,
    PaginatedCoins,
    PaginatedEvents,
    PaginatedObjectsResponse,
    PaginatedTransactionResponse,
    ProtocolConfig,
    QueryEventsParams,
    QueryTransactionBlocksParams,
    ResolvedNameServiceNames,
    SubscribeEventParams,
    SubscribeTransactionParams,
    IotaEvent,
    IotaMoveFunctionArgType,
    IotaMoveNormalizedFunction,
    IotaMoveNormalizedModule,
    IotaMoveNormalizedModules,
    IotaMoveNormalizedStruct,
    IotaObjectResponse,
    IotaObjectResponseQuery,
    IotaSystemStateSummary,
    IotaTransactionBlockResponse,
    IotaTransactionBlockResponseQuery,
    TransactionEffects,
    TryGetPastObjectParams,
    Unsubscribe,
    ValidatorsApy,
    GetTimelockedStakesParams,
    DelegatedTimelockedStake,
    GetTimelockedStakesByIdsParams,
} from './types/index.js';

export interface PaginationArguments<Cursor> {
    /** Optional paging cursor */
    cursor?: Cursor;
    /** Maximum item returned per page */
    limit?: number | null;
}

export interface OrderArguments {
    order?: Order | null;
}

/**
 * Configuration options for the IotaClient
 * You must provide either a `url` or a `transport`
 */
export type IotaClientOptions = NetworkOrTransport;

export type NetworkOrTransport =
    | {
          url: string;
          transport?: never;
      }
    | {
          transport: IotaTransport;
          url?: never;
      };

export const IOTA_CLIENT_BRAND = Symbol.for('@iota/IotaClient');

export function isIotaClient(client: unknown): client is IotaClient {
    return (
        typeof client === 'object' &&
        client !== null &&
        (client as { [IOTA_CLIENT_BRAND]: unknown })[IOTA_CLIENT_BRAND] === true
    );
}

export class IotaClient {
    protected transport: IotaTransport;

    get [IOTA_CLIENT_BRAND]() {
        return true;
    }

    /**
     * Establish a connection to a Iota RPC endpoint
     *
     * @param options configuration options for the API Client
     */
    constructor(options: IotaClientOptions) {
        this.transport = options.transport ?? new IotaHTTPTransport({ url: options.url });
    }

    async getRpcApiVersion(): Promise<string | undefined> {
        const resp = await this.transport.request<{ info: { version: string } }>({
            method: 'rpc.discover',
            params: [],
        });

        return resp.info.version;
    }

    /**
     * Get all Coin<`coin_type`> objects owned by an address.
     */
    async getCoins(input: GetCoinsParams): Promise<PaginatedCoins> {
        if (!input.owner || !isValidIotaAddress(normalizeIotaAddress(input.owner))) {
            throw new Error('Invalid Iota address');
        }

        return await this.transport.request({
            method: 'iotax_getCoins',
            params: [input.owner, input.coinType, input.cursor, input.limit],
        });
    }

    /**
     * Get all Coin objects owned by an address.
     */
    async getAllCoins(input: GetAllCoinsParams): Promise<PaginatedCoins> {
        if (!input.owner || !isValidIotaAddress(normalizeIotaAddress(input.owner))) {
            throw new Error('Invalid Iota address');
        }

        return await this.transport.request({
            method: 'iotax_getAllCoins',
            params: [input.owner, input.cursor, input.limit],
        });
    }

    /**
     * Get the total coin balance for one coin type, owned by the address owner.
     */
    async getBalance(input: GetBalanceParams): Promise<CoinBalance> {
        if (!input.owner || !isValidIotaAddress(normalizeIotaAddress(input.owner))) {
            throw new Error('Invalid Iota address');
        }
        return await this.transport.request({
            method: 'iotax_getBalance',
            params: [input.owner, input.coinType],
        });
    }

    /**
     * Get the total coin balance for all coin types, owned by the address owner.
     */
    async getAllBalances(input: GetAllBalancesParams): Promise<CoinBalance[]> {
        if (!input.owner || !isValidIotaAddress(normalizeIotaAddress(input.owner))) {
            throw new Error('Invalid Iota address');
        }
        return await this.transport.request({
            method: 'iotax_getAllBalances',
            params: [input.owner],
        });
    }

    /**
     * Fetch CoinMetadata for a given coin type
     */
    async getCoinMetadata(input: GetCoinMetadataParams): Promise<CoinMetadata | null> {
        return await this.transport.request({
            method: 'iotax_getCoinMetadata',
            params: [input.coinType],
        });
    }

    /**
     *  Fetch total supply for a coin
     */
    async getTotalSupply(input: GetTotalSupplyParams): Promise<CoinSupply> {
        return await this.transport.request({
            method: 'iotax_getTotalSupply',
            params: [input.coinType],
        });
    }

    /**
     * Invoke any RPC method
     * @param method the method to be invoked
     * @param args the arguments to be passed to the RPC request
     */
    async call<T = unknown>(method: string, params: unknown[]): Promise<T> {
        return await this.transport.request({ method, params });
    }

    /**
     * Get Move function argument types like read, write and full access
     */
    async getMoveFunctionArgTypes(
        input: GetMoveFunctionArgTypesParams,
    ): Promise<IotaMoveFunctionArgType[]> {
        return await this.transport.request({
            method: 'iota_getMoveFunctionArgTypes',
            params: [input.package, input.module, input.function],
        });
    }

    /**
     * Get a map from module name to
     * structured representations of Move modules
     */
    async getNormalizedMoveModulesByPackage(
        input: GetNormalizedMoveModulesByPackageParams,
    ): Promise<IotaMoveNormalizedModules> {
        return await this.transport.request({
            method: 'iota_getNormalizedMoveModulesByPackage',
            params: [input.package],
        });
    }

    /**
     * Get a structured representation of Move module
     */
    async getNormalizedMoveModule(
        input: GetNormalizedMoveModuleParams,
    ): Promise<IotaMoveNormalizedModule> {
        return await this.transport.request({
            method: 'iota_getNormalizedMoveModule',
            params: [input.package, input.module],
        });
    }

    /**
     * Get a structured representation of Move function
     */
    async getNormalizedMoveFunction(
        input: GetNormalizedMoveFunctionParams,
    ): Promise<IotaMoveNormalizedFunction> {
        return await this.transport.request({
            method: 'iota_getNormalizedMoveFunction',
            params: [input.package, input.module, input.function],
        });
    }

    /**
     * Get a structured representation of Move struct
     */
    async getNormalizedMoveStruct(
        input: GetNormalizedMoveStructParams,
    ): Promise<IotaMoveNormalizedStruct> {
        return await this.transport.request({
            method: 'iota_getNormalizedMoveStruct',
            params: [input.package, input.module, input.struct],
        });
    }

    /**
     * Get all objects owned by an address
     */
    async getOwnedObjects(input: GetOwnedObjectsParams): Promise<PaginatedObjectsResponse> {
        if (!input.owner || !isValidIotaAddress(normalizeIotaAddress(input.owner))) {
            throw new Error('Invalid Iota address');
        }

        return await this.transport.request({
            method: 'iotax_getOwnedObjects',
            params: [
                input.owner,
                {
                    filter: input.filter,
                    options: input.options,
                } as IotaObjectResponseQuery,
                input.cursor,
                input.limit,
            ],
        });
    }

    /**
     * Get details about an object
     */
    async getObject(input: GetObjectParams): Promise<IotaObjectResponse> {
        if (!input.id || !isValidIotaObjectId(normalizeIotaObjectId(input.id))) {
            throw new Error('Invalid Iota Object id');
        }
        return await this.transport.request({
            method: 'iota_getObject',
            params: [input.id, input.options],
        });
    }

    async tryGetPastObject(input: TryGetPastObjectParams): Promise<ObjectRead> {
        return await this.transport.request({
            method: 'iota_tryGetPastObject',
            params: [input.id, input.version, input.options],
        });
    }

    /**
     * Batch get details about a list of objects. If any of the object ids are duplicates the call will fail
     */
    async multiGetObjects(input: MultiGetObjectsParams): Promise<IotaObjectResponse[]> {
        input.ids.forEach((id) => {
            if (!id || !isValidIotaObjectId(normalizeIotaObjectId(id))) {
                throw new Error(`Invalid Iota Object id ${id}`);
            }
        });
        const hasDuplicates = input.ids.length !== new Set(input.ids).size;
        if (hasDuplicates) {
            throw new Error(`Duplicate object ids in batch call ${input.ids}`);
        }

        return await this.transport.request({
            method: 'iota_multiGetObjects',
            params: [input.ids, input.options],
        });
    }

    /**
     * Get transaction blocks for a given query criteria
     */
    async queryTransactionBlocks(
        input: QueryTransactionBlocksParams,
    ): Promise<PaginatedTransactionResponse> {
        return await this.transport.request({
            method: 'iotax_queryTransactionBlocks',
            params: [
                {
                    filter: input.filter,
                    options: input.options,
                } as IotaTransactionBlockResponseQuery,
                input.cursor,
                input.limit,
                (input.order || 'descending') === 'descending',
            ],
        });
    }

    async getTransactionBlock(
        input: GetTransactionBlockParams,
    ): Promise<IotaTransactionBlockResponse> {
        if (!isValidTransactionDigest(input.digest)) {
            throw new Error('Invalid Transaction digest');
        }
        return await this.transport.request({
            method: 'iota_getTransactionBlock',
            params: [input.digest, input.options],
        });
    }

    async multiGetTransactionBlocks(
        input: MultiGetTransactionBlocksParams,
    ): Promise<IotaTransactionBlockResponse[]> {
        input.digests.forEach((d) => {
            if (!isValidTransactionDigest(d)) {
                throw new Error(`Invalid Transaction digest ${d}`);
            }
        });

        const hasDuplicates = input.digests.length !== new Set(input.digests).size;
        if (hasDuplicates) {
            throw new Error(`Duplicate digests in batch call ${input.digests}`);
        }

        return await this.transport.request({
            method: 'iota_multiGetTransactionBlocks',
            params: [input.digests, input.options],
        });
    }

    async executeTransactionBlock(
        input: ExecuteTransactionBlockParams,
    ): Promise<IotaTransactionBlockResponse> {
        return await this.transport.request({
            method: 'iota_executeTransactionBlock',
            params: [
                typeof input.transactionBlock === 'string'
                    ? input.transactionBlock
                    : toB64(input.transactionBlock),
                Array.isArray(input.signature) ? input.signature : [input.signature],
                input.options,
                input.requestType,
            ],
        });
    }

    async signAndExecuteTransactionBlock({
        transactionBlock,
        signer,
        ...input
    }: {
        transactionBlock: Uint8Array | TransactionBlock;
        signer: Signer;
    } & Omit<
        ExecuteTransactionBlockParams,
        'transactionBlock' | 'signature'
    >): Promise<IotaTransactionBlockResponse> {
        let transactionBytes;

        if (transactionBlock instanceof Uint8Array) {
            transactionBytes = transactionBlock;
        } else {
            transactionBlock.setSenderIfNotSet(signer.toIotaAddress());
            transactionBytes = await transactionBlock.build({ client: this });
        }

        const { signature, bytes } = await signer.signTransactionBlock(transactionBytes);

        return this.executeTransactionBlock({
            transactionBlock: bytes,
            signature,
            ...input,
        });
    }

    /**
     * Get total number of transactions
     */

    async getTotalTransactionBlocks(): Promise<bigint> {
        const resp = await this.transport.request<string>({
            method: 'iota_getTotalTransactionBlocks',
            params: [],
        });
        return BigInt(resp);
    }

    /**
     * Getting the reference gas price for the network
     */
    async getReferenceGasPrice(): Promise<bigint> {
        const resp = await this.transport.request<string>({
            method: 'iotax_getReferenceGasPrice',
            params: [],
        });
        return BigInt(resp);
    }

    /**
     * Return the delegated stakes for an address
     */
    async getStakes(input: GetStakesParams): Promise<DelegatedStake[]> {
        if (!input.owner || !isValidIotaAddress(normalizeIotaAddress(input.owner))) {
            throw new Error('Invalid Iota address');
        }
        return await this.transport.request({ method: 'iotax_getStakes', params: [input.owner] });
    }

    /**
     * Return the timelocked delegated stakes for an address
     */
    async getTimelockedStakes(
        input: GetTimelockedStakesParams,
    ): Promise<DelegatedTimelockedStake[]> {
        if (!input.owner || !isValidIotaAddress(normalizeIotaAddress(input.owner))) {
            throw new Error('Invalid Iota address');
        }
        return await this.transport.request({
            method: 'iotax_getTimelockedStakes',
            params: [input.owner],
        });
    }

    /**
     * Return the delegated stakes queried by id.
     */
    async getStakesByIds(input: GetStakesByIdsParams): Promise<DelegatedStake[]> {
        input.stakedIotaIds.forEach((id) => {
            if (!id || !isValidIotaObjectId(normalizeIotaObjectId(id))) {
                throw new Error(`Invalid Iota Stake id ${id}`);
            }
        });
        return await this.transport.request({
            method: 'iotax_getStakesByIds',
            params: [input.stakedIotaIds],
        });
    }

    /**
     * Return the timelocked delegated stakes queried by id.
     */
    async getTimelockedStakesByIds(
        input: GetTimelockedStakesByIdsParams,
    ): Promise<DelegatedTimelockedStake[]> {
        input.timelockedStakedIotaIds.forEach((id) => {
            if (!id || !isValidIotaObjectId(normalizeIotaObjectId(id))) {
                throw new Error(`Invalid Iota Timelocked Stake id ${id}`);
            }
        });
        return await this.transport.request({
            method: 'iotax_getTimelockedStakesByIds',
            params: [input.timelockedStakedIotaIds],
        });
    }

    /**
     * Return the latest system state content.
     */
    async getLatestIotaSystemState(): Promise<IotaSystemStateSummary> {
        return await this.transport.request({
            method: 'iotax_getLatestIotaSystemState',
            params: [],
        });
    }

    /**
     * Get events for a given query criteria
     */
    async queryEvents(input: QueryEventsParams): Promise<PaginatedEvents> {
        return await this.transport.request({
            method: 'iotax_queryEvents',
            params: [
                input.query,
                input.cursor,
                input.limit,
                (input.order || 'descending') === 'descending',
            ],
        });
    }

    /**
     * Subscribe to get notifications whenever an event matching the filter occurs
     */
    async subscribeEvent(
        input: SubscribeEventParams & {
            /** function to run when we receive a notification of a new event matching the filter */
            onMessage: (event: IotaEvent) => void;
        },
    ): Promise<Unsubscribe> {
        return this.transport.subscribe({
            method: 'iotax_subscribeEvent',
            unsubscribe: 'iotax_unsubscribeEvent',
            params: [input.filter],
            onMessage: input.onMessage,
        });
    }

    async subscribeTransaction(
        input: SubscribeTransactionParams & {
            /** function to run when we receive a notification of a new event matching the filter */
            onMessage: (event: TransactionEffects) => void;
        },
    ): Promise<Unsubscribe> {
        return this.transport.subscribe({
            method: 'iotax_subscribeTransaction',
            unsubscribe: 'iotax_unsubscribeTransaction',
            params: [input.filter],
            onMessage: input.onMessage,
        });
    }

    /**
     * Runs the transaction block in dev-inspect mode. Which allows for nearly any
     * transaction (or Move call) with any arguments. Detailed results are
     * provided, including both the transaction effects and any return values.
     */
    async devInspectTransactionBlock(
        input: DevInspectTransactionBlockParams,
    ): Promise<DevInspectResults> {
        let devInspectTxBytes;
        if (isTransactionBlock(input.transactionBlock)) {
            input.transactionBlock.setSenderIfNotSet(input.sender);
            devInspectTxBytes = toB64(
                await input.transactionBlock.build({
                    client: this,
                    onlyTransactionKind: true,
                }),
            );
        } else if (typeof input.transactionBlock === 'string') {
            devInspectTxBytes = input.transactionBlock;
        } else if (input.transactionBlock instanceof Uint8Array) {
            devInspectTxBytes = toB64(input.transactionBlock);
        } else {
            throw new Error('Unknown transaction block format.');
        }

        return await this.transport.request({
            method: 'iota_devInspectTransactionBlock',
            params: [input.sender, devInspectTxBytes, input.gasPrice?.toString(), input.epoch],
        });
    }

    /**
     * Dry run a transaction block and return the result.
     */
    async dryRunTransactionBlock(
        input: DryRunTransactionBlockParams,
    ): Promise<DryRunTransactionBlockResponse> {
        return await this.transport.request({
            method: 'iota_dryRunTransactionBlock',
            params: [
                typeof input.transactionBlock === 'string'
                    ? input.transactionBlock
                    : toB64(input.transactionBlock),
            ],
        });
    }

    /**
     * Return the list of dynamic field objects owned by an object
     */
    async getDynamicFields(input: GetDynamicFieldsParams): Promise<DynamicFieldPage> {
        if (!input.parentId || !isValidIotaObjectId(normalizeIotaObjectId(input.parentId))) {
            throw new Error('Invalid Iota Object id');
        }
        return await this.transport.request({
            method: 'iotax_getDynamicFields',
            params: [input.parentId, input.cursor, input.limit],
        });
    }

    /**
     * Return the dynamic field object information for a specified object
     */
    async getDynamicFieldObject(input: GetDynamicFieldObjectParams): Promise<IotaObjectResponse> {
        return await this.transport.request({
            method: 'iotax_getDynamicFieldObject',
            params: [input.parentId, input.name],
        });
    }

    /**
     * Get the sequence number of the latest checkpoint that has been executed
     */
    async getLatestCheckpointSequenceNumber(): Promise<string> {
        const resp = await this.transport.request({
            method: 'iota_getLatestCheckpointSequenceNumber',
            params: [],
        });
        return String(resp);
    }

    /**
     * Returns information about a given checkpoint
     */
    async getCheckpoint(input: GetCheckpointParams): Promise<Checkpoint> {
        return await this.transport.request({ method: 'iota_getCheckpoint', params: [input.id] });
    }

    /**
     * Returns historical checkpoints paginated
     */
    async getCheckpoints(
        input: PaginationArguments<CheckpointPage['nextCursor']> & GetCheckpointsParams,
    ): Promise<CheckpointPage> {
        return await this.transport.request({
            method: 'iota_getCheckpoints',
            params: [input.cursor, input?.limit, input.descendingOrder],
        });
    }

    /**
     * Return the committee information for the asked epoch
     */
    async getCommitteeInfo(input?: GetCommitteeInfoParams): Promise<CommitteeInfo> {
        return await this.transport.request({
            method: 'iotax_getCommitteeInfo',
            params: [input?.epoch],
        });
    }

    async getNetworkMetrics(): Promise<NetworkMetrics> {
        return await this.transport.request({ method: 'iotax_getNetworkMetrics', params: [] });
    }

    async getAddressMetrics(): Promise<AddressMetrics> {
        return await this.transport.request({
            method: 'iotax_getLatestAddressMetrics',
            params: [],
        });
    }

    async getEpochMetrics(
        input?: { descendingOrder?: boolean } & PaginationArguments<EpochMetricsPage['nextCursor']>,
    ): Promise<EpochMetricsPage> {
        return await this.transport.request({
            method: 'iotax_getEpochMetrics',
            params: [input?.cursor, input?.limit, input?.descendingOrder],
        });
    }

    async getAllEpochAddressMetrics(input?: {
        descendingOrder?: boolean;
    }): Promise<AllEpochsAddressMetrics> {
        return await this.transport.request({
            method: 'iotax_getAllEpochAddressMetrics',
            params: [input?.descendingOrder],
        });
    }

    /**
     * Return the committee information for the asked epoch
     */
    async getEpochs(
        input?: {
            descendingOrder?: boolean;
        } & PaginationArguments<EpochPage['nextCursor']>,
    ): Promise<EpochPage> {
        return await this.transport.request({
            method: 'iotax_getEpochs',
            params: [input?.cursor, input?.limit, input?.descendingOrder],
        });
    }

    /**
     * Returns list of top move calls by usage
     */
    async getMoveCallMetrics(): Promise<MoveCallMetrics> {
        return await this.transport.request({ method: 'iotax_getMoveCallMetrics', params: [] });
    }

    /**
     * Return the committee information for the asked epoch
     */
    async getCurrentEpoch(): Promise<EpochInfo> {
        return await this.transport.request({ method: 'iotax_getCurrentEpoch', params: [] });
    }

    /**
     * Return the Validators APYs
     */
    async getValidatorsApy(): Promise<ValidatorsApy> {
        return await this.transport.request({ method: 'iotax_getValidatorsApy', params: [] });
    }

    // TODO: Migrate this to `iota_getChainIdentifier` once it is widely available.
    async getChainIdentifier(): Promise<string> {
        const checkpoint = await this.getCheckpoint({ id: '0' });
        const bytes = fromB58(checkpoint.digest);
        return toHEX(bytes.slice(0, 4));
    }

    async resolveNameServiceAddress(_input: any): Promise<string | null> {
        return 'remove_me';
    }

    async resolveNameServiceNames(_input: any): Promise<ResolvedNameServiceNames> {
        return {
            data: [],
            hasNextPage: false,
            nextCursor: null,
        };
    }

    async getProtocolConfig(input?: GetProtocolConfigParams): Promise<ProtocolConfig> {
        return await this.transport.request({
            method: 'iota_getProtocolConfig',
            params: [input?.version],
        });
    }

    /**
     * Wait for a transaction block result to be available over the API.
     * This can be used in conjunction with `executeTransactionBlock` to wait for the transaction to
     * be available via the API.
     * This currently polls the `getTransactionBlock` API to check for the transaction.
     */
    async waitForTransactionBlock({
        signal,
        timeout = 60 * 1000,
        pollInterval = 2 * 1000,
        ...input
    }: {
        /** An optional abort signal that can be used to cancel */
        signal?: AbortSignal;
        /** The amount of time to wait for a transaction block. Defaults to one minute. */
        timeout?: number;
        /** The amount of time to wait between checks for the transaction block. Defaults to 2 seconds. */
        pollInterval?: number;
    } & Parameters<IotaClient['getTransactionBlock']>[0]): Promise<IotaTransactionBlockResponse> {
        const timeoutSignal = AbortSignal.timeout(timeout);
        const timeoutPromise = new Promise((_, reject) => {
            timeoutSignal.addEventListener('abort', () => reject(timeoutSignal.reason));
        });

        timeoutPromise.catch(() => {
            // Swallow unhandled rejections that might be thrown after early return
        });

        while (!timeoutSignal.aborted) {
            signal?.throwIfAborted();
            try {
                return await this.getTransactionBlock(input);
            } catch (e) {
                // Wait for either the next poll interval, or the timeout.
                await Promise.race([
                    new Promise((resolve) => setTimeout(resolve, pollInterval)),
                    timeoutPromise,
                ]);
            }
        }

        timeoutSignal.throwIfAborted();

        // This should never happen, because the above case should always throw, but just adding it in the event that something goes horribly wrong.
        throw new Error('Unexpected error while waiting for transaction block.');
    }
}
