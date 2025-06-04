// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { toB64 } from '@iota/bcs';

import type { Signer } from '../cryptography/index.js';
import type { Transaction } from '../transactions/index.js';
import { isTransaction } from '../transactions/index.js';
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
    IotaSystemStateSummaryV1,
    LatestIotaSystemStateSummary,
    ParticipationMetrics,
    IotaCirculatingSupply,
    GetDynamicFieldObjectV2Params,
    IotaNamesLookupParams,
    IotaNameRecord,
    IotaNamesReverseLookupParams,
    IotaNamesFindAllRegistrationNFTsParams,
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

type NetworkOrTransport =
    | {
          url: string;
          transport?: never;
      }
    | {
          transport: IotaTransport;
          url?: never;
      };

const IOTA_CLIENT_BRAND = Symbol.for('@iota/IotaClient') as never;

export function isIotaClient(client: unknown): client is IotaClient {
    return (
        typeof client === 'object' && client !== null && (client as any)[IOTA_CLIENT_BRAND] === true
    );
}

export class IotaClient {
    protected transport: IotaTransport;

    get [IOTA_CLIENT_BRAND]() {
        return true;
    }

    /**
     * Establish a connection to an IOTA RPC endpoint
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
            throw new Error('Invalid IOTA address');
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
            throw new Error('Invalid IOTA address');
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
            throw new Error('Invalid IOTA address');
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
            throw new Error('Invalid IOTA address');
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
     *  Fetch circulating supply for a coin
     */
    async getCirculatingSupply(): Promise<IotaCirculatingSupply> {
        return await this.transport.request({
            method: 'iotax_getCirculatingSupply',
            params: [],
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
            throw new Error('Invalid IOTA address');
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
            throw new Error('Invalid IOTA Object id');
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
                throw new Error(`Invalid IOTA Object id ${id}`);
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

    async executeTransactionBlock({
        transactionBlock,
        signature,
        options,
    }: ExecuteTransactionBlockParams): Promise<IotaTransactionBlockResponse> {
        const result: IotaTransactionBlockResponse = await this.transport.request({
            method: 'iota_executeTransactionBlock',
            params: [
                typeof transactionBlock === 'string' ? transactionBlock : toB64(transactionBlock),
                Array.isArray(signature) ? signature : [signature],
                options,
            ],
        });

        return result;
    }

    async signAndExecuteTransaction({
        transaction,
        signer,
        ...input
    }: {
        transaction: Uint8Array | Transaction;
        signer: Signer;
    } & Omit<
        ExecuteTransactionBlockParams,
        'transactionBlock' | 'signature'
    >): Promise<IotaTransactionBlockResponse> {
        let transactionBytes;

        if (transaction instanceof Uint8Array) {
            transactionBytes = transaction;
        } else {
            transaction.setSenderIfNotSet(signer.toIotaAddress());
            transactionBytes = await transaction.build({ client: this });
        }

        const { signature, bytes } = await signer.signTransaction(transactionBytes);

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
            throw new Error('Invalid IOTA address');
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
            throw new Error('Invalid IOTA address');
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
                throw new Error(`Invalid IOTA Stake id ${id}`);
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
                throw new Error(`Invalid IOTA Timelocked Stake id ${id}`);
            }
        });
        return await this.transport.request({
            method: 'iotax_getTimelockedStakesByIds',
            params: [input.timelockedStakedIotaIds],
        });
    }

    /**
     * Return the latest IOTA system state object on networks supporting protocol version `< 5`.
     * These are networks with node software release version `< 0.11`.
     * @deprecated Use `getLatestIotaSystemState` instead.
     */
    async getLatestIotaSystemStateV1(): Promise<IotaSystemStateSummaryV1> {
        return await this.transport.request({
            method: 'iotax_getLatestIotaSystemState',
            params: [],
        });
    }

    /**
     * Return the latest IOTA system state object on networks supporting protocol version `>= 5`.
     * These are networks with node software release version `>= 0.11`.
     *
     * You probably want to use `getLatestIotaSystemState` instead to prevent issues with future deprecations
     * or in case the node does not support protocol version `>= 5`.
     */
    async getLatestIotaSystemStateV2(): Promise<IotaSystemStateSummary> {
        return await this.transport.request<IotaSystemStateSummary>({
            method: 'iotax_getLatestIotaSystemStateV2',
            params: [],
        });
    }

    /**
     * Return the latest supported IOTA system state object.
     *
     * This returns a backwards-compatible system state object that dynamically uses the V1 or V2
     * depending on the protocol version supported by the node. This method will continue to be supported
     * as more protocol versions are released with changes to the system state.
     *
     * This is quite useful in case your app does not know in advance what node is it going to be using,
     * this way you as developer dont need to handle each possible system state variant,
     * this is already handled by this method.
     */
    async getLatestIotaSystemState(): Promise<LatestIotaSystemStateSummary> {
        const protocolConfig = await this.getProtocolConfig();
        const isV2Supported = Number(protocolConfig.maxSupportedProtocolVersion) >= 5;

        const iotaSystemStateSummary: IotaSystemStateSummary = isV2Supported
            ? await this.getLatestIotaSystemStateV2()
            : {
                  V1: await this.getLatestIotaSystemStateV1(),
              };

        return 'V2' in iotaSystemStateSummary
            ? {
                  ...iotaSystemStateSummary.V2,
                  committeeMembers: iotaSystemStateSummary.V2.committeeMembers.map(
                      (committeeMemberIndex) =>
                          iotaSystemStateSummary.V2.activeValidators[Number(committeeMemberIndex)],
                  ),
              }
            : {
                  ...iotaSystemStateSummary.V1,
                  committeeMembers: iotaSystemStateSummary.V1.activeValidators,
                  safeModeComputationCharges: iotaSystemStateSummary.V1.safeModeComputationRewards,
                  safeModeComputationChargesBurned:
                      iotaSystemStateSummary.V1.safeModeComputationRewards,
              };
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
     *
     * @deprecated
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

    /**
     * @deprecated
     */
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
        if (isTransaction(input.transactionBlock)) {
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
            throw new Error('Invalid IOTA Object id');
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
     * Return the dynamic field object information for a specified object with content options.
     */
    async getDynamicFieldObjectV2(
        input: GetDynamicFieldObjectV2Params,
    ): Promise<IotaObjectResponse> {
        return await this.transport.request({
            method: 'iotax_getDynamicFieldObjectV2',
            params: [input.parentObjectId, input.name],
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

    async getCheckpointAddressMetrics(input?: { checkpoint: string }): Promise<AddressMetrics> {
        return await this.transport.request({
            method: 'iotax_getCheckpointAddressMetrics',
            params: [input?.checkpoint],
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

    async getTotalTransactions(): Promise<string> {
        const resp = await this.transport.request({
            method: 'iotax_getTotalTransactions',
            params: [],
        });
        return String(resp);
    }

    /**
     * Return the Validators APYs
     */
    async getValidatorsApy(): Promise<ValidatorsApy> {
        return await this.transport.request({ method: 'iotax_getValidatorsApy', params: [] });
    }

    async getChainIdentifier(): Promise<string> {
        return await this.transport.request({
            method: 'iota_getChainIdentifier',
            params: [],
        });
    }

    async getProtocolConfig(input?: GetProtocolConfigParams): Promise<ProtocolConfig> {
        return await this.transport.request({
            method: 'iota_getProtocolConfig',
            params: [input?.version],
        });
    }

    /**
     * Returns the participation metrics (total unique addresses with delegated stake in the current epoch).
     */
    async getParticipationMetrics(): Promise<ParticipationMetrics> {
        return await this.transport.request({
            method: 'iotax_getParticipationMetrics',
            params: [],
        });
    }

    /**
     * Wait for a transaction block result to be available over the API.
     * This can be used in conjunction with `executeTransactionBlock` to wait for the transaction to
     * be available via the API.
     * This currently polls the `getTransactionBlock` API to check for the transaction.
     */
    async waitForTransaction({
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

    /**
     * Return the resolved record for the given name.
     */
    async iotaNamesLookup(input: IotaNamesLookupParams): Promise<IotaNameRecord | undefined> {
        return await this.transport.request({
            method: 'iotax_iotaNamesLookup',
            params: [input.name],
        });
    }

    /**
     * Return the resolved name for the given address.
     */
    async iotaNamesReverseLookup(input: IotaNamesReverseLookupParams): Promise<string | undefined> {
        return await this.transport.request({
            method: 'iotax_iotaNamesReverseLookup',
            params: [input.address],
        });
    }

    /**
     * Find all registration NFTs for the given address.
     */
    async iotaNamesFindAllRegistrationNFTs(
        input: IotaNamesFindAllRegistrationNFTsParams,
    ): Promise<PaginatedObjectsResponse> {
        return await this.transport.request({
            method: 'iotax_iotaNamesFindAllRegistrationNFTs',
            params: [input.address, input.cursor, input.limit, input.options],
        });
    }
}
