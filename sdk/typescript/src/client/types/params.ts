// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 *  ######################################
 *  ### DO NOT EDIT THIS FILE DIRECTLY ###
 *  ######################################
 *
 * This file is generated from:
 * /crates/iota-open-rpc/spec/openrpc.json
 */

import type { Transaction } from '../../transactions/index.js';
import type * as RpcTypes from './generated.js';

/**
 * Runs the transaction in dev-inspect mode. Which allows for nearly any transaction (or Move call)
 * with any arguments. Detailed results are provided, including both the transaction effects and any
 * return values.
 */
export interface DevInspectTransactionBlockParams {
	sender: string;
	/** BCS encoded TransactionKind(as opposed to TransactionData, which include gasBudget and gasPrice) */
	transactionBlock: Transaction | Uint8Array | string;
	/** Gas is not charged, but gas usage is still calculated. Default to use reference gas price */
	gasPrice?: bigint | number | null | undefined;
	/** The epoch to perform the call. Will be set from the system state object if not provided */
	epoch?: string | null | undefined;
	/** Additional arguments including gas_budget, gas_objects, gas_sponsor and skip_checks. */
	additionalArgs?: RpcTypes.DevInspectArgs | null | undefined;
}
/**
 * Return transaction execution effects including the gas cost summary, while the effects are not
 * committed to the chain.
 */
export interface DryRunTransactionBlockParams {
	transactionBlock: Uint8Array | string;
}
/**
 * Execute the transaction and wait for results if desired. Request types: 1. WaitForEffectsCert: waits
 * for TransactionEffectsCert and then return to client. This mode is a proxy for transaction
 * finality. 2. WaitForLocalExecution: waits for TransactionEffectsCert and make sure the node executed
 * the transaction locally before returning the client. The local execution makes sure this node is
 * aware of this transaction when client fires subsequent queries. However if the node fails to execute
 * the transaction locally in a timely manner, a bool type in the response is set to false to indicated
 * the case. request_type is default to be `WaitForEffectsCert` unless options.show_events or
 * options.show_effects is true
 */
export interface ExecuteTransactionBlockParams {
	/** BCS serialized transaction data bytes without its type tag, as base-64 encoded string. */
	transactionBlock: Uint8Array | string;
	/**
	 * A list of signatures (`flag || signature || pubkey` bytes, as base-64 encoded string). Signature is
	 * committed to the intent message of the transaction data, as base-64 encoded string.
	 */
	signature: string | string[];
	/** options for specifying the content to be returned */
	options?: RpcTypes.IotaTransactionBlockResponseOptions | null | undefined;
	/** @deprecated requestType will be ignored by JSON RPC in the future */
	requestType?: RpcTypes.ExecuteTransactionRequestType | null | undefined;
}
/** Return the first four bytes of the chain's genesis checkpoint digest. */
export interface GetChainIdentifierParams {}
/** Return a checkpoint */
export interface GetCheckpointParams {
	/** Checkpoint identifier, can use either checkpoint digest, or checkpoint sequence number as input. */
	id: RpcTypes.CheckpointId;
}
/** Return paginated list of checkpoints */
export interface GetCheckpointsParams {
	/**
	 * An optional paging cursor. If provided, the query will start from the next item after the specified
	 * cursor. Default to start from the first item if not specified.
	 */
	cursor?: string | null | undefined;
	/** Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT_CHECKPOINTS] if not specified. */
	limit?: number | null | undefined;
	/** query result ordering, default to false (ascending order), oldest record first. */
	descendingOrder: boolean;
}
/** Return transaction events. */
export interface GetEventsParams {
	/** the event query criteria. */
	transactionDigest: string;
}
/** Return the sequence number of the latest checkpoint that has been executed */
export interface GetLatestCheckpointSequenceNumberParams {}
/** Return the argument types of a Move function, based on normalized Type. */
export interface GetMoveFunctionArgTypesParams {
	package: string;
	module: string;
	function: string;
}
/** Return a structured representation of Move function */
export interface GetNormalizedMoveFunctionParams {
	package: string;
	module: string;
	function: string;
}
/** Return a structured representation of Move module */
export interface GetNormalizedMoveModuleParams {
	package: string;
	module: string;
}
/** Return structured representations of all modules in the given package */
export interface GetNormalizedMoveModulesByPackageParams {
	package: string;
}
/** Return a structured representation of Move struct */
export interface GetNormalizedMoveStructParams {
	package: string;
	module: string;
	struct: string;
}
/** Return the object information for a specified object */
export interface GetObjectParams {
	/** the ID of the queried object */
	id: string;
	/** options for specifying the content to be returned */
	options?: RpcTypes.IotaObjectDataOptions | null | undefined;
}
/**
 * Return the protocol config table for the given version number. If the version number is not
 * specified, If none is specified, the node uses the version of the latest epoch it has processed.
 */
export interface GetProtocolConfigParams {
	/**
	 * An optional protocol version specifier. If omitted, the latest protocol config table for the node
	 * will be returned.
	 */
	version?: string | null | undefined;
}
/** Return the total number of transaction blocks known to the server. */
export interface GetTotalTransactionBlocksParams {}
/** Return the transaction response object. */
export interface GetTransactionBlockParams {
	/** the digest of the queried transaction */
	digest: string;
	/** options for specifying the content to be returned */
	options?: RpcTypes.IotaTransactionBlockResponseOptions | null | undefined;
}
/** Return the object data for a list of objects */
export interface MultiGetObjectsParams {
	/** the IDs of the queried objects */
	ids: string[];
	/** options for specifying the content to be returned */
	options?: RpcTypes.IotaObjectDataOptions | null | undefined;
}
/**
 * Returns an ordered list of transaction responses The method will throw an error if the input
 * contains any duplicate or the input size exceeds QUERY_MAX_RESULT_LIMIT
 */
export interface MultiGetTransactionBlocksParams {
	/** A list of transaction digests. */
	digests: string[];
	/** config options to control which fields to fetch */
	options?: RpcTypes.IotaTransactionBlockResponseOptions | null | undefined;
}
/**
 * Note there is no software-level guarantee/SLA that objects with past versions can be retrieved by
 * this API, even if the object and version exists/existed. The result may vary across nodes depending
 * on their pruning policies. Return the object information for a specified version
 */
export interface TryGetPastObjectParams {
	/** the ID of the queried object */
	id: string;
	/** the version of the queried object. If None, default to the latest known version */
	version: number;
	/** options for specifying the content to be returned */
	options?: RpcTypes.IotaObjectDataOptions | null | undefined;
}
/**
 * Note there is no software-level guarantee/SLA that objects with past versions can be retrieved by
 * this API, even if the object and version exists/existed. The result may vary across nodes depending
 * on their pruning policies. Return the object information for a specified version
 */
export interface TryMultiGetPastObjectsParams {
	/** a vector of object and versions to be queried */
	pastObjects: RpcTypes.GetPastObjectRequest[];
	/** options for specifying the content to be returned */
	options?: RpcTypes.IotaObjectDataOptions | null | undefined;
}
/** Return the total coin balance for all coin type, owned by the address owner. */
export interface GetAllBalancesParams {
	/** the owner's Iota address */
	owner: string;
}
/** Return all Coin objects owned by an address. */
export interface GetAllCoinsParams {
	/** the owner's Iota address */
	owner: string;
	/** optional paging cursor */
	cursor?: string | null | undefined;
	/** maximum number of items per page */
	limit?: number | null | undefined;
}
/** Return the total coin balance for one coin type, owned by the address owner. */
export interface GetBalanceParams {
	/** the owner's Iota address */
	owner: string;
	/**
	 * optional type names for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC),
	 * default to 0x2::iota::IOTA if not specified.
	 */
	coinType?: string | null | undefined;
}
/** Return metadata(e.g., symbol, decimals) for a coin */
export interface GetCoinMetadataParams {
	/** type name for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC) */
	coinType: string;
}
/** Return all Coin<`coin_type`> objects owned by an address. */
export interface GetCoinsParams {
	/** the owner's Iota address */
	owner: string;
	/**
	 * optional type name for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC),
	 * default to 0x2::iota::IOTA if not specified.
	 */
	coinType?: string | null | undefined;
	/** optional paging cursor */
	cursor?: string | null | undefined;
	/** maximum number of items per page */
	limit?: number | null | undefined;
}
/** Return the committee information for the asked `epoch`. */
export interface GetCommitteeInfoParams {
	/** The epoch of interest. If None, default to the latest epoch */
	epoch?: string | null | undefined;
}
/** Return the dynamic field object information for a specified object */
export interface GetDynamicFieldObjectParams {
	/** The ID of the queried parent object */
	parentId: string;
	/** The Name of the dynamic field */
	name: RpcTypes.DynamicFieldName;
}
/** Return the list of dynamic field objects owned by an object. */
export interface GetDynamicFieldsParams {
	/** The ID of the parent object */
	parentId: string;
	/**
	 * An optional paging cursor. If provided, the query will start from the next item after the specified
	 * cursor. Default to start from the first item if not specified.
	 */
	cursor?: string | null | undefined;
	/** Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified. */
	limit?: number | null | undefined;
}
/** Return the latest IOTA system state object on-chain. */
export interface GetLatestIotaSystemStateParams {}
/**
 * Return the list of objects owned by an address. Note that if the address owns more than
 * `QUERY_MAX_RESULT_LIMIT` objects, the pagination is not accurate, because previous page may have
 * been updated when the next page is fetched. Please use iotax_queryObjects if this is a concern.
 */
export type GetOwnedObjectsParams = {
	/** the owner's Iota address */
	owner: string;
	/**
	 * An optional paging cursor. If provided, the query will start from the next item after the specified
	 * cursor. Default to start from the first item if not specified.
	 */
	cursor?: string | null | undefined;
	/** Max number of items returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified. */
	limit?: number | null | undefined;
} & RpcTypes.IotaObjectResponseQuery;
/** Return the reference gas price for the network */
export interface GetReferenceGasPriceParams {}
/** Return all [DelegatedStake]. */
export interface GetStakesParams {
	owner: string;
}
/** Return one or more [DelegatedStake]. If a Stake was withdrawn its status will be Unstaked. */
export interface GetStakesByIdsParams {
	stakedIotaIds: string[];
}
/** Return total supply for a coin */
export interface GetTotalSupplyParams {
	/** type name for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC) */
	coinType: string;
}
/** Return the validator APY */
export interface GetValidatorsApyParams {}
/** Return list of events for a specified query criteria. */
export interface QueryEventsParams {
	/**
	 * The event query criteria. See [Event filter](https://docs.iota.org/build/event_api#event-filters)
	 * documentation for examples.
	 */
	query: RpcTypes.IotaEventFilter;
	/** optional paging cursor */
	cursor?: RpcTypes.EventId | null | undefined;
	/** maximum number of items per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified. */
	limit?: number | null | undefined;
	/** query result ordering, default to false (ascending order), oldest record first. */
	order?: 'ascending' | 'descending' | null | undefined;
}
/** Return list of transactions for a specified query criteria. */
export type QueryTransactionBlocksParams = {
	/**
	 * An optional paging cursor. If provided, the query will start from the next item after the specified
	 * cursor. Default to start from the first item if not specified.
	 */
	cursor?: string | null | undefined;
	/** Maximum item returned per page, default to QUERY_MAX_RESULT_LIMIT if not specified. */
	limit?: number | null | undefined;
	/** query result ordering, default to false (ascending order), oldest record first. */
	order?: 'ascending' | 'descending' | null | undefined;
} & RpcTypes.IotaTransactionBlockResponseQuery;
/** Return the resolved address given resolver and name */
export interface ResolveNameServiceAddressParams {
	/** The name to resolve */
	name: string;
}
/**
 * Return the resolved names given address, if multiple names are resolved, the first one is the
 * primary name.
 */
export interface ResolveNameServiceNamesParams {
	/** The address to resolve */
	address: string;
	cursor?: string | null | undefined;
	limit?: number | null | undefined;
}
/** Subscribe to a stream of Iota event */
export interface SubscribeEventParams {
	/**
	 * The filter criteria of the event stream. See
	 * [Event filter](https://docs.iota.org/build/event_api#event-filters) documentation for examples.
	 */
	filter: RpcTypes.IotaEventFilter;
}
/** Subscribe to a stream of Iota transaction effects */
export interface SubscribeTransactionParams {
	filter: RpcTypes.TransactionFilter;
}
/** Create an unsigned batched transaction. */
export interface UnsafeBatchTransactionParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** list of transaction request parameters */
	singleTransactionParams: RpcTypes.RPCTransactionRequestParams[];
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
	/** Whether this is a regular transaction or a Dev Inspect Transaction */
	txnBuilderMode?: RpcTypes.IotaTransactionBlockBuilderMode | null | undefined;
}
/** Create an unsigned transaction to merge multiple coins into one coin. */
export interface UnsafeMergeCoinsParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the coin object to merge into, this coin will remain after the transaction */
	primaryCoin: string;
	/**
	 * the coin object to be merged, this coin will be destroyed, the balance will be added to
	 * `primary_coin`
	 */
	coinToMerge: string;
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/**
 * Create an unsigned transaction to execute a Move call on the network, by calling the specified
 * function in the module of a given package.
 */
export interface UnsafeMoveCallParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the Move package ID, e.g. `0x2` */
	packageObjectId: string;
	/** the Move module name, e.g. `pay` */
	module: string;
	/** the move function name, e.g. `split` */
	function: string;
	/** the type arguments of the Move function */
	typeArguments: string[];
	/**
	 * the arguments to be passed into the Move function, in [IotaJson](https://docs.iota.org/build/iota-json)
	 * format
	 */
	arguments: unknown[];
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
	/**
	 * Whether this is a Normal transaction or a Dev Inspect Transaction. Default to be
	 * `IotaTransactionBlockBuilderMode::Commit` when it's None.
	 */
	executionMode?: RpcTypes.IotaTransactionBlockBuilderMode | null | undefined;
}
/**
 * Send `Coin<T>` to a list of addresses, where `T` can be any coin type, following a list of amounts,
 * The object specified in the `gas` field will be used to pay the gas fee for the transaction. The gas
 * object can not appear in `input_coins`. If the gas object is not specified, the RPC server will
 * auto-select one.
 */
export interface UnsafePayParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the Iota coins to be used in this transaction */
	inputCoins: string[];
	/** the recipients' addresses, the length of this vector must be the same as amounts. */
	recipients: string[];
	/** the amounts to be transferred to recipients, following the same order */
	amounts: string[];
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/**
 * Send all IOTA coins to one recipient. This is for IOTA coin only and does not require a separate gas
 * coin object. Specifically, what pay_all_iota does are: 1. accumulate all IOTA from input coins and
 * deposit all IOTA to the first input coin 2. transfer the updated first coin to the recipient and also
 * use this first coin as gas coin object. 3. the balance of the first input coin after tx is
 * sum(input_coins) - actual_gas_cost. 4. all other input coins other than the first are deleted.
 */
export interface UnsafePayAllIotaParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the Iota coins to be used in this transaction, including the coin for gas payment. */
	inputCoins: string[];
	/** the recipient address, */
	recipient: string;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/**
 * Send IOTA coins to a list of addresses, following a list of amounts. This is for IOTA coin only and
 * does not require a separate gas coin object. Specifically, what pay_iota does are: 1. debit each
 * input_coin to create new coin following the order of amounts and assign it to the corresponding
 * recipient. 2. accumulate all residual IOTA from input coins left and deposit all IOTA to the first
 * input coin, then use the first input coin as the gas coin object. 3. the balance of the first input
 * coin after tx is sum(input_coins) - sum(amounts) - actual_gas_cost 4. all other input coints other
 * than the first one are deleted.
 */
export interface UnsafePayIotaParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the Iota coins to be used in this transaction, including the coin for gas payment. */
	inputCoins: string[];
	/** the recipients' addresses, the length of this vector must be the same as amounts. */
	recipients: string[];
	/** the amounts to be transferred to recipients, following the same order */
	amounts: string[];
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/** Create an unsigned transaction to publish a Move package. */
export interface UnsafePublishParams {
	/** the transaction signer's Iota address */
	sender: string;
	/** the compiled bytes of a Move package */
	compiledModules: string[];
	/** a list of transitive dependency addresses that this set of modules depends on. */
	dependencies: string[];
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/** Add stake to a validator's staking pool using multiple coins and amount. */
export interface UnsafeRequestAddStakeParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** Coin<IOTA> object to stake */
	coins: string[];
	/** stake amount */
	amount?: string | null | undefined;
	/** the validator's Iota address */
	validator: string;
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/** Withdraw stake from a validator's staking pool. */
export interface UnsafeRequestWithdrawStakeParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** StakedIota object ID */
	stakedIota: string;
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/** Create an unsigned transaction to split a coin object into multiple coins. */
export interface UnsafeSplitCoinParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the coin object to be spilt */
	coinObjectId: string;
	/** the amounts to split out from the coin */
	splitAmounts: string[];
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/** Create an unsigned transaction to split a coin object into multiple equal-size coins. */
export interface UnsafeSplitCoinEqualParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the coin object to be spilt */
	coinObjectId: string;
	/** the number of coins to split into */
	splitCount: string;
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
}
/**
 * Create an unsigned transaction to transfer an object from one address to another. The object's type
 * must allow public transfers
 */
export interface UnsafeTransferObjectParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the ID of the object to be transferred */
	objectId: string;
	/**
	 * gas object to be used in this transaction, node will pick one from the signer's possession if not
	 * provided
	 */
	gas?: string | null | undefined;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
	/** the recipient's Iota address */
	recipient: string;
}
/**
 * Create an unsigned transaction to send IOTA coin object to a Iota address. The IOTA object is also used
 * as the gas object.
 */
export interface UnsafeTransferIotaParams {
	/** the transaction signer's Iota address */
	signer: string;
	/** the Iota coin object to be used in this transaction */
	iotaObjectId: string;
	/** the gas budget, the transaction will fail if the gas cost exceed the budget */
	gasBudget: string;
	/** the recipient's Iota address */
	recipient: string;
	/** the amount to be split out and transferred */
	amount?: string | null | undefined;
}
