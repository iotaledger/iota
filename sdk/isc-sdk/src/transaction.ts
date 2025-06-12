import { Transaction, TransactionObjectArgument } from '@iota/iota-sdk/transactions';
import * as isc from './isc';
import { ChainData } from './types';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { L2_FROM_L1_GAS_BUDGET } from './constants';
import type { ObjectArgument } from './isc';

export type Agent = {
    type: 'evm';
    address: string;
};

export class IscTransaction {
    #finalized: boolean;
    #transaction: Transaction;
    #chainData: ChainData;

    constructor(chainData: ChainData, transaction = new Transaction()) {
        this.#finalized = false;
        this.#transaction = transaction;
        this.#chainData = chainData;
    }

    private validateFinalizedStatus() {
        if (this.#finalized) {
            throw Error('Transaction is built.');
        }
    }

    /**
     * Getter for the IOTA MOVE Transaction.
     */
    transaction(): Transaction {
        return this.#transaction;
    }

    /**
     * Create a bag.
     */
    newBag(): TransactionObjectArgument {
        this.validateFinalizedStatus();
        return isc.newBag(this.#transaction, this.#chainData);
    }

    /**
     * Get some amount in a coin.
     */
    coinFromAmount({ amount }: { amount: number | bigint }) {
        this.validateFinalizedStatus();
        return isc.coinFromAmount(this.#transaction, BigInt(amount));
    }

    /**
     * Place a coin in a bag.
     *
     * **Uses the IOTA Coin Type by default.**
     */
    placeCoinInBag({
        bag,
        coinType = IOTA_TYPE_ARG,
        coin,
    }: {
        coin: ObjectArgument;
        coinType?: string;
        bag: TransactionObjectArgument;
    }) {
        this.validateFinalizedStatus();
        isc.placeCoinInBag(this.#transaction, this.#chainData, bag, coinType, coin);
    }

    /**
     * Finally create and send a request calling the given `contractFunction` with `contractArgs` in `contract`
     */
    createAndSend({
        bag,
        contract,
        contractFunction,
        contractArgs,
        transfers,
        gasBudget = L2_FROM_L1_GAS_BUDGET,
    }: {
        contractArgs: Uint8Array[];
        contract: number;
        contractFunction: number;
        transfers: Array<[string, number | bigint]>;
        gasBudget?: number | bigint;
        bag: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        isc.createAndSendRequest(
            this.#transaction,
            this.#chainData,
            contract,
            contractFunction,
            contractArgs,
            bag,
            transfers,
            gasBudget,
        );
    }

    /**
     * Finally create and send a request calling the given `accountsFunction` in `accountsContract`
     */
    createAndSendToEvm({
        address,
        accountsContract,
        accountsFunction,
        transfers,
        gasBudget = L2_FROM_L1_GAS_BUDGET,
        bag,
    }: {
        address: string;
        accountsContract: number;
        accountsFunction: number;
        transfers: Array<[string, number | bigint]>;
        gasBudget?: number | bigint;
        bag: ObjectArgument;
    }) {
        const agentID = isc.agentIdForEVM(address);
        this.createAndSend({
            bag,
            gasBudget,
            contract: accountsContract,
            contractFunction: accountsFunction,
            contractArgs: [agentID],
            transfers,
        });
    }

    /**
     * Take out the specified amount of coin from the bag.
     *
     * **Uses the IOTA Coin Type by default.**
     */
    takeCoinBalanceFromBag({
        bag,
        coinType = IOTA_TYPE_ARG,
        amount,
    }: {
        amount: number | bigint;
        coinType?: string;
        bag: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        return isc.takeCoinBalanceFromBag(
            this.#transaction,
            this.#chainData,
            bag,
            coinType,
            amount,
        );
    }

    /**
     * Take out all the coin from the bag.
     *
     * **Uses the IOTA Coin Type by default.**
     */
    takeAllCoinBalanceFromBag({
        bag,
        coinType = IOTA_TYPE_ARG,
    }: {
        bag: ObjectArgument;
        coinType?: string;
    }) {
        this.validateFinalizedStatus();
        return isc.takeAllCoinBalanceFromBag(this.#transaction, this.#chainData, bag, coinType);
    }

    /**
     * Place a coin balance in the bag.
     *
     * **Uses the IOTA Coin Type by default.**
     */
    placeCoinBalanceInBag({
        bag,
        coinType = IOTA_TYPE_ARG,
        balance,
    }: {
        balance: ObjectArgument;
        coinType?: string;
        bag: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        isc.placeCoinBalanceInBag(this.#transaction, this.#chainData, bag, coinType, balance);
    }

    /**
     * Place an asset in the bag.
     */
    placeAssetInBag({
        bag,
        asset,
        assetType,
    }: {
        asset: ObjectArgument;
        bag: ObjectArgument;
        assetType: string;
    }) {
        this.validateFinalizedStatus();
        isc.placeAssetInBag(this.#transaction, this.#chainData, bag, assetType, asset);
    }

    /**
     * Take an asset from a bag.
     */
    takeAssetFromBag({
        bag,
        assetType,
        asset,
    }: {
        bag: ObjectArgument;
        assetType: string;
        asset: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        isc.takeAssetFromBag(this.#transaction, this.#chainData, bag, assetType, asset);
    }

    /**
     * Get the size of the bag.
     */
    getSizeOfBag({ bag }: { bag: ObjectArgument }) {
        this.validateFinalizedStatus();
        return isc.getSizeOfBag(this.#transaction, this.#chainData, bag);
    }

    /**
     * Destroy the bag.
     */
    destroyBag({ bag }: { bag: ObjectArgument }) {
        this.validateFinalizedStatus();
        return isc.destroyBag(this.#transaction, this.#chainData, bag);
    }

    startNewChain({ metadata, coin }: { metadata: Uint8Array; coin?: ObjectArgument }) {
        this.validateFinalizedStatus();
        return isc.startNewChain(this.#transaction, this.#chainData, metadata, coin);
    }

    createAnchorWithAssetBag({ bag }: { bag: ObjectArgument }) {
        this.validateFinalizedStatus();
        return isc.createAnchorWithAssetBag(this.#transaction, this.#chainData, bag);
    }

    updateAnchorStateForMigraton({
        anchor,
        metadata,
        stateIndex,
    }: {
        anchor: ObjectArgument;
        metadata: Uint8Array;
        stateIndex: number;
    }) {
        this.validateFinalizedStatus();
        return isc.updateAnchorStateForMigraton(
            this.#transaction,
            this.#chainData,
            anchor,
            metadata,
            stateIndex,
        );
    }

    destroyAnchor({ anchor }: { anchor: ObjectArgument }) {
        this.validateFinalizedStatus();
        return isc.destroyAnchor(this.#transaction, this.#chainData, anchor);
    }

    borrowAssets({ anchor }: { anchor: ObjectArgument }) {
        this.validateFinalizedStatus();
        return isc.borrowAssets(this.#transaction, this.#chainData, anchor);
    }

    returnAssetsFromBorrow({
        anchor,
        bag,
        borrow,
    }: {
        anchor: ObjectArgument;
        bag: ObjectArgument;
        borrow: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        return isc.returnAssetsFromBorrow(this.#transaction, this.#chainData, anchor, bag, borrow);
    }

    receiveRequest({ anchor, request }: { anchor: ObjectArgument; request: ObjectArgument }) {
        this.validateFinalizedStatus();
        return isc.receiveRequest(this.#transaction, this.#chainData, anchor, request);
    }

    transition({
        anchor,
        newStateMetadata,
        receipts,
    }: {
        anchor: ObjectArgument;
        newStateMetadata: Uint8Array;
        receipts: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        return isc.transition(
            this.#transaction,
            this.#chainData,
            anchor,
            newStateMetadata,
            receipts,
        );
    }

    placeCoinForMigration({
        anchor,
        coinType = IOTA_TYPE_ARG,
        coin,
    }: {
        anchor: ObjectArgument;
        coinType?: string;
        coin: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        return isc.placeCoinForMigration(
            this.#transaction,
            this.#chainData,
            anchor,
            coinType,
            coin,
        );
    }

    placeCoinBalanceForMigration({
        anchor,
        coinType = IOTA_TYPE_ARG,
        balance,
    }: {
        anchor: ObjectArgument;
        coinType?: string;
        balance: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        return isc.placeCoinBalanceForMigration(
            this.#transaction,
            this.#chainData,
            anchor,
            coinType,
            balance,
        );
    }

    placeAssetForMigration({
        anchor,
        assetType,
        asset,
    }: {
        anchor: ObjectArgument;
        assetType: string;
        asset: ObjectArgument;
    }) {
        this.validateFinalizedStatus();
        return isc.placeAssetForMigration(
            this.#transaction,
            this.#chainData,
            anchor,
            assetType,
            asset,
        );
    }

    /**
     * Stop building this ISC Transaction and return the IOTA MOVE Transaction.
     * @returns IOTA MOVE Transaction.
     */
    build(): Transaction {
        this.validateFinalizedStatus();
        this.#finalized = true;
        return this.#transaction;
    }
}
