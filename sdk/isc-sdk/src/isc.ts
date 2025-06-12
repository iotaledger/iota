import { Transaction, TransactionObjectArgument } from '@iota/iota-sdk/transactions';
import { ChainData } from './types';
import { bcs } from '@iota/iota-sdk/bcs';
import { IscAgentID, IscAssets } from './bcs';

export type ObjectArgument = string | TransactionObjectArgument;

export function newBag(tx: Transaction, { packageId }: ChainData): TransactionObjectArgument {
    // Create a new empty AssetsBag. It will be used to attach coin/objects to the request
    const assetsBag = tx.moveCall({
        target: `${packageId}::assets_bag::new`,
        arguments: [],
    });

    return assetsBag;
}

export function coinFromAmount(tx: Transaction, amount: bigint): TransactionObjectArgument {
    // Split the senders Gas coin so we have a coin to transfer
    const [splitCoin] = tx.splitCoins(tx.gas, [tx.pure(bcs.U64.serialize(amount))]);

    return splitCoin;
}

export function placeCoinInBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
    coinType: string,
    coin: ObjectArgument,
) {
    tx.moveCall({
        target: `${packageId}::assets_bag::place_coin`,
        typeArguments: [coinType],
        arguments: [tx.object(assetsBag), tx.object(coin)],
    });
}

export function agentIdForEVM(address: string): Uint8Array {
    const agentID = IscAgentID.serialize({
        EthereumAddressAgentID: {
            eth: bcs.fixedArray(20, bcs.u8()).fromHex(address),
        },
    });

    return agentID.toBytes();
}

export function createAndSendRequest(
    tx: Transaction,
    { packageId, chainId }: ChainData,
    contract: number,
    contractFunction: number,
    contractArgs: Uint8Array[],
    assetsBag: ObjectArgument,
    transfers: Array<[string, number | bigint]>,
    gasBudget: number | bigint,
) {
    // Encodes the allowance.
    //  coins:      map[coinType(string): balance(u64)]
    //  objects:    map[objectID(bytes32): objectType(string)]
    //
    //  Example:
    //      {
    //          coins: {
    //              "0x02::iota::IOTA": 1074
    //          },
    //          objects: {
    //              fromHex('0x629aeef09ab0874db9b9d9dbf8098ef9e1d4f466ca7569c4ad18d1db4b0e9e7b'): "0xca99629453167d3c4d754ac11d23132a510094addb344cbaea306483a72658c2::anchor::Anchor"
    //          }
    //      }
    const allowance = IscAssets.serialize({
        coins: new Map(transfers.map(([coinType, amount]) => [coinType, Number(amount)])),
        objects: new Map(), // Add objects here. Provide their ID as BCS encoded bytes *and* the _object type_ as string.
    });

    // Execute requests::create_and_send_request.
    // This creates the Request Move object and sends it to the Anchor object of the Chain (ChainID == Anchor Object ID)
    tx.moveCall({
        target: `${packageId}::request::create_and_send_request`,
        arguments: [
            tx.pure(bcs.Address.serialize(chainId)),
            tx.object(assetsBag),
            tx.pure(bcs.U32.serialize(contract)),
            tx.pure(bcs.U32.serialize(contractFunction)),
            tx.pure(bcs.vector(bcs.vector(bcs.u8())).serialize(contractArgs)),
            tx.pure(bcs.vector(bcs.u8()).serialize(allowance.toBytes())),
            tx.pure(bcs.U64.serialize(gasBudget)),
        ],
    });
}

export function takeCoinBalanceFromBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
    coinType: string,
    amount: number | bigint,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::assets_bag::take_coin_balance`,
        typeArguments: [coinType],
        arguments: [tx.object(assetsBag), tx.pure(bcs.U64.serialize(amount))],
    });
}

export function takeAllCoinBalanceFromBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
    coinType: string,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::assets_bag::take_all_coin_balance`,
        typeArguments: [coinType],
        arguments: [tx.object(assetsBag)],
    });
}

export function placeCoinBalanceInBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
    coinType: string,
    balance: ObjectArgument,
) {
    tx.moveCall({
        target: `${packageId}::assets_bag::place_coin_balance`,
        typeArguments: [coinType],
        arguments: [tx.object(assetsBag), tx.object(balance)],
    });
}

export function placeAssetInBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
    assetType: string,
    asset: ObjectArgument,
) {
    tx.moveCall({
        target: `${packageId}::assets_bag::place_asset`,
        typeArguments: [assetType],
        arguments: [tx.object(assetsBag), tx.object(asset)],
    });
}

export function takeAssetFromBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
    assetType: string,
    asset: ObjectArgument,
) {
    return tx.moveCall({
        target: `${packageId}::assets_bag::take_asset`,
        typeArguments: [assetType],
        arguments: [tx.object(assetsBag), tx.object(asset)],
    });
}

export function getSizeOfBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::assets_bag::get_size`,
        arguments: [tx.object(assetsBag)],
    });
}

export function destroyBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::assets_bag::destroy_empty`,
        arguments: [tx.object(assetsBag)],
    });
}

export function startNewChain(
    tx: Transaction,
    { packageId }: ChainData,
    metadata: Uint8Array,
    coin?: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::start_new_chain`,
        arguments: [
            bcs.vector(bcs.u8()).serialize(metadata),
            coin ? tx.object(coin) : bcs.option(bcs.ObjectArg).serialize(null),
        ],
    });
}

export function createAnchorWithAssetBag(
    tx: Transaction,
    { packageId }: ChainData,
    assetsBag: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::create_anchor_with_assets_bag_ref`,
        arguments: [tx.object(assetsBag)],
    });
}

export function updateAnchorStateForMigraton(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
    metadata: Uint8Array,
    stateIndex: number,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::update_anchor_state_for_migration`,
        arguments: [
            tx.object(anchor),
            bcs.vector(bcs.u8()).serialize(metadata),
            bcs.u32().serialize(stateIndex),
        ],
    });
}

export function destroyAnchor(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::destroy`,
        arguments: [tx.object(anchor)],
    });
}

export function borrowAssets(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::borrow_assets`,
        arguments: [tx.object(anchor)],
    });
}

export function returnAssetsFromBorrow(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
    assetsBag: ObjectArgument,
    borrow: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::return_assets_from_borrow`,
        arguments: [tx.object(anchor), tx.object(assetsBag), tx.object(borrow)],
    });
}

export function receiveRequest(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
    request: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::receive_request`,
        arguments: [tx.object(anchor), tx.object(request)],
    });
}

export function transition(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
    newStateMetadata: Uint8Array,
    receipts: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::transition`,
        arguments: [
            tx.object(anchor),
            bcs.vector(bcs.u8()).serialize(newStateMetadata),
            tx.object(receipts),
        ],
    });
}

export function placeCoinForMigration(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
    coinType: string,
    coin: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::place_coin_for_migration`,
        typeArguments: [coinType],
        arguments: [tx.object(anchor), tx.object(coin)],
    });
}

export function placeCoinBalanceForMigration(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
    coinType: string,
    balance: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::place_coin_balance_for_migration`,
        typeArguments: [coinType],
        arguments: [tx.object(anchor), tx.object(balance)],
    });
}

export function placeAssetForMigration(
    tx: Transaction,
    { packageId }: ChainData,
    anchor: ObjectArgument,
    assetType: string,
    asset: ObjectArgument,
): TransactionObjectArgument {
    return tx.moveCall({
        target: `${packageId}::anchor::place_asset_for_migration`,
        typeArguments: [assetType],
        arguments: [tx.object(anchor), tx.object(asset)],
    });
}
