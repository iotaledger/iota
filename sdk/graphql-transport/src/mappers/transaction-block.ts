// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromB64, toB58 } from '@iota/bcs';
import { bcs } from '@iota/iota-sdk/bcs';
import type {
    IotaArgument,
    IotaCallArg,
    IotaObjectChange,
    IotaTransaction,
    IotaTransactionBlock,
    IotaTransactionBlockKind,
    IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions,
} from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';

import type { Rpc_Transaction_FieldsFragment } from '../generated/queries.js';
import { toShortTypeString } from './util.js';

export function mapGraphQLTransactionBlockToRpcTransactionBlock(
    transactionBlock: Rpc_Transaction_FieldsFragment,
    options?: IotaTransactionBlockResponseOptions | null,
    errors?: string[] | null,
): IotaTransactionBlockResponse {
    const effects = transactionBlock.effects?.bcs ? mapEffects(transactionBlock.effects.bcs) : null;

    return {
        balanceChanges: transactionBlock.effects?.balanceChanges?.nodes.map((balanceChange) => ({
            amount: balanceChange?.amount,
            coinType: toShortTypeString(balanceChange?.coinType?.repr),
            owner: balanceChange.owner?.asObject?.address
                ? {
                      ObjectOwner: balanceChange.owner?.asObject?.address,
                  }
                : {
                      AddressOwner: balanceChange.owner?.asAddress?.address!,
                  },
        })),
        ...(typeof transactionBlock.effects?.checkpoint?.sequenceNumber === 'number'
            ? { checkpoint: transactionBlock.effects.checkpoint.sequenceNumber.toString() }
            : {}),
        ...(transactionBlock.effects?.timestamp
            ? { timestampMs: new Date(transactionBlock.effects?.timestamp).getTime().toString() }
            : {}),
        digest: transactionBlock.digest!,
        ...(options?.showRawEffects
            ? {
                  rawEffects: transactionBlock.effects?.bcs
                      ? Array.from(fromB64(transactionBlock.effects?.bcs))
                      : undefined,
              }
            : {}),
        effects: options?.showEffects ? effects : undefined,
        ...(errors ? { errors: errors } : {}),
        events:
            transactionBlock.effects?.events?.nodes.map((event) => ({
                bcs: event.bcs,
                id: {
                    eventSeq: '', // TODO
                    txDigest: '', // TODO
                },
                packageId: event.sendingModule?.package.address!,
                parsedJson: event.json ? JSON.parse(event.json) : undefined,
                sender: event.sender?.address,
                timestampMs: new Date(event.timestamp).getTime().toString(),
                transactionModule: `${event.sendingModule?.package.address}::${event.sendingModule?.name}`,
                type: toShortTypeString(event.type?.repr)!,
            })) ?? [],
        rawTransaction: options?.showRawInput ? transactionBlock.rawTransaction : undefined,
        ...(options?.showInput
            ? {
                  transaction:
                      transactionBlock.rawTransaction &&
                      mapTransactionBlockToInput(
                          bcs.SenderSignedData.parse(fromB64(transactionBlock.rawTransaction))[0],
                      ),
              }
            : {}),
        objectChanges: options?.showObjectChanges
            ? mapObjectChanges(transactionBlock, effects)
            : undefined,
    };
}

function mapObjectChanges(
    transactionBlock: Rpc_Transaction_FieldsFragment,
    effects: IotaTransactionBlockResponse['effects'],
) {
    const changes: IotaObjectChange[] = [];

    effects?.mutated?.forEach((mutated) => {
        const objectChange = transactionBlock.effects?.objectChanges?.nodes.find(
            (change) => change.address === mutated.reference.objectId,
        );
        changes.push({
            type: 'mutated',
            digest: mutated.reference.digest,
            previousVersion: String(objectChange?.inputState?.version),
            objectId: mutated.reference.objectId,
            owner: mutated.owner,
            objectType: toShortTypeString(
                objectChange?.outputState?.asMoveObject?.contents?.type.repr!,
            ),
            sender: transactionBlock.sender?.address!,
            version: mutated.reference.version?.toString(),
        });
    });

    effects?.created?.forEach((created) => {
        const objectChange = transactionBlock.effects?.objectChanges?.nodes.find(
            (change) => change.address === created.reference.objectId,
        );

        if (objectChange?.outputState?.asMovePackage) {
            changes.push({
                type: 'published',
                digest: created.reference.digest,
                version: created.reference.version?.toString(),
                packageId: objectChange.address,
                modules: objectChange.outputState.asMovePackage.modules?.nodes.map(
                    (module) => module.name,
                )!,
            });
        } else {
            changes.push({
                type: 'created',
                digest: created.reference.digest,
                objectId: created.reference.objectId,
                owner: created.owner,
                objectType: toShortTypeString(
                    transactionBlock.effects?.objectChanges?.nodes.find(
                        (change) => change.address === created.reference.objectId,
                    )?.outputState?.asMoveObject?.contents?.type.repr!,
                ),
                sender: transactionBlock.sender?.address!,
                version: created.reference.version?.toString(),
            });
        }
    });

    effects?.deleted?.forEach((deleted) => {
        changes.push({
            type: 'deleted',
            objectId: deleted.objectId,
            objectType: toShortTypeString(
                transactionBlock.effects?.objectChanges?.nodes.find(
                    (change) => change.address === deleted.objectId,
                )?.inputState?.asMoveObject?.contents?.type.repr!,
            ),
            sender: transactionBlock.sender?.address!,
            version: deleted.version?.toString(),
        });
    });

    effects?.unwrapped?.forEach((unwrapped) => {
        changes.push({
            type: 'wrapped',
            objectId: unwrapped.reference.objectId,
            objectType: toShortTypeString(
                transactionBlock.effects?.objectChanges?.nodes.find(
                    (change) => change.address === unwrapped.reference.objectId,
                )?.outputState?.asMoveObject?.contents?.type.repr!,
            ),
            sender: transactionBlock.sender?.address!,
            version: unwrapped.reference.version?.toString(),
        });
    });

    return changes;
}

export function mapTransactionBlockToInput(
    data: typeof bcs.SenderSignedTransaction.$inferType,
): IotaTransactionBlock | null {
    const txData = data.intentMessage.value.V1;

    const programableTransaction =
        'ProgrammableTransaction' in txData.kind ? txData.kind.ProgrammableTransaction : null;

    if (!programableTransaction) {
        return null;
    }

    return {
        txSignatures: data.txSignatures,
        data: {
            gasData: {
                budget: txData.gasData.budget,
                owner: txData.gasData.owner,
                payment: txData.gasData.payment.map((payment) => ({
                    digest: payment.digest,
                    objectId: payment.objectId,
                    version: Number(payment.version) as never as string,
                })),
                price: txData.gasData.price,
            },
            messageVersion: 'v1',
            sender: txData.sender,
            transaction: mapProgramableTransaction(programableTransaction),
        },
    };
}

export function mapProgramableTransaction(
    programableTransaction: typeof bcs.ProgrammableTransaction.$inferType,
): IotaTransactionBlockKind {
    return {
        inputs: programableTransaction.inputs.map(mapTransactionInput),
        kind: 'ProgrammableTransaction',
        transactions: programableTransaction.commands.map(mapTransaction),
    };
}

function mapTransactionInput(input: typeof bcs.CallArg.$inferType): IotaCallArg {
    if (input.Pure) {
        return {
            type: 'pure',
            value: fromB64(input.Pure.bytes),
        };
    }

    if (input.Object.ImmOrOwnedObject) {
        return {
            type: 'object',
            digest: input.Object.ImmOrOwnedObject.digest,
            version: input.Object.ImmOrOwnedObject.version,
            objectId: input.Object.ImmOrOwnedObject.objectId,
            objectType: 'immOrOwnedObject',
        };
    }
    if (input.Object.SharedObject) {
        return {
            type: 'object',
            initialSharedVersion: input.Object.SharedObject.initialSharedVersion,
            objectId: input.Object.SharedObject.objectId,
            mutable: input.Object.SharedObject.mutable,
            objectType: 'sharedObject',
        };
    }

    if (input.Object.Receiving) {
        return {
            type: 'object',
            digest: input.Object.Receiving.digest,
            version: input.Object.Receiving.version,
            objectId: input.Object.Receiving.objectId,
            objectType: 'receiving',
        };
    }

    throw new Error(`Unknown object type: ${input.Object}`);
}

function mapTransaction(transaction: typeof bcs.Command.$inferType): IotaTransaction {
    switch (transaction.$kind) {
        case 'MoveCall': {
            return {
                MoveCall: {
                    arguments: transaction.MoveCall.arguments.map(mapTransactionArgument),
                    function: transaction.MoveCall.function,
                    module: transaction.MoveCall.module,
                    package: transaction.MoveCall.package,
                    type_arguments: transaction.MoveCall.typeArguments,
                },
            };
        }

        case 'MakeMoveVec': {
            return {
                MakeMoveVec: [
                    transaction.MakeMoveVec.type,
                    transaction.MakeMoveVec.elements.map(mapTransactionArgument),
                ],
            };
        }
        case 'MergeCoins': {
            return {
                MergeCoins: [
                    mapTransactionArgument(transaction.MergeCoins.destination),
                    transaction.MergeCoins.sources.map(mapTransactionArgument),
                ],
            };
        }
        case 'Publish': {
            return {
                Publish: transaction.Publish.modules.map((module) => module),
            };
        }
        case 'SplitCoins': {
            return {
                SplitCoins: [
                    mapTransactionArgument(transaction.SplitCoins.coin),
                    transaction.SplitCoins.amounts.map(mapTransactionArgument),
                ],
            };
        }
        case 'TransferObjects': {
            return {
                TransferObjects: [
                    transaction.TransferObjects.objects.map(mapTransactionArgument),
                    mapTransactionArgument(transaction.TransferObjects.address),
                ],
            };
        }
        case 'Upgrade': {
            return {
                Upgrade: [
                    transaction.Upgrade.modules.map((module) => module),
                    transaction.Upgrade.package,
                    mapTransactionArgument(transaction.Upgrade.ticket),
                ],
            };
        }
    }

    throw new Error(`Unknown transaction type ${transaction}`);
}

function mapTransactionArgument(arg: typeof bcs.Argument.$inferType): IotaArgument {
    switch (arg.$kind) {
        case 'GasCoin': {
            return 'GasCoin';
        }
        case 'Input': {
            return {
                Input: arg.Input,
            };
        }
        case 'Result': {
            return {
                Result: arg.Result,
            };
        }
        case 'NestedResult': {
            return {
                NestedResult: arg.NestedResult,
            };
        }
    }

    throw new Error(`Unknown argument type ${arg}`);
}

const OBJECT_DIGEST_DELETED = toB58(Uint8Array.from({ length: 32 }, () => 99));
const OBJECT_DIGEST_WRAPPED = toB58(Uint8Array.from({ length: 32 }, () => 88));
const OBJECT_DIGEST_ZERO = toB58(Uint8Array.from({ length: 32 }, () => 0));
const ADDRESS_ZERO = normalizeIotaAddress('0x0');

export function mapEffects(data: string): IotaTransactionBlockResponse['effects'] {
    const effects = bcs.TransactionEffects.parse(fromB64(data));

    // let effectsV1 = effects.V1;

    const sharedObjects = effects.V1.unchangedSharedObjects.map(([id, sharedObject]) => {
        switch (sharedObject.$kind) {
            case 'ReadOnlyRoot':
                return {
                    objectId: id,
                    version: Number(sharedObject.ReadOnlyRoot[0]) as unknown as string,
                    digest: sharedObject.ReadOnlyRoot[1],
                };
            case 'MutateDeleted':
                return {
                    objectId: id,
                    version: Number(sharedObject.MutateDeleted) as unknown as string,
                    digest: OBJECT_DIGEST_DELETED,
                };
            case 'ReadDeleted':
                return {
                    objectId: id,
                    version: Number(sharedObject.ReadDeleted) as unknown as string,
                    digest: OBJECT_DIGEST_DELETED,
                };
            default:
                throw new Error(`Unknown shared object type: ${sharedObject}`);
        }
    });

    effects.V1.changedObjects
        .filter(([_id, change]) => change.inputState.Exist?.[1].Shared)
        .forEach(([id, change]) => {
            sharedObjects.push({
                objectId: id,
                version: Number(change.inputState.Exist![0][0]) as unknown as string,
                digest: change.inputState.Exist![0][1],
            });
        });

    const modifiedAtVersions = effects.V1.changedObjects
        .filter(([_id, change]) => change.inputState.Exist)
        .map(([id, change]) => [id, change.inputState.Exist![0][0]] as const);

    const created = effects.V1.changedObjects
        .filter(
            ([_id, change]) =>
                change.inputState.NotExist &&
                (change.outputState.ObjectWrite || change.outputState.PackageWrite) &&
                change.idOperation.Created,
        )
        .map(([objectId, change]) =>
            change.outputState.PackageWrite
                ? ([
                      {
                          objectId,
                          version: Number(change.outputState.PackageWrite[0]) as unknown as string,
                          digest: change.outputState.PackageWrite[1],
                      },
                      { $kind: 'Immutable', Immutable: true },
                  ] as const)
                : ([
                      {
                          objectId,
                          version: Number(effects.V1.lamportVersion) as unknown as string,
                          digest: change.outputState.ObjectWrite![0],
                      },
                      change.outputState.ObjectWrite![1],
                  ] as const),
        );

    const mutated = effects.V1.changedObjects
        .filter(
            ([_id, change]) =>
                change.inputState.Exist &&
                (change.outputState.ObjectWrite || change.outputState.PackageWrite),
        )
        .map(([objectId, change]) => [
            change.outputState.PackageWrite
                ? {
                      objectId,
                      version: Number(change.outputState.PackageWrite[0]) as unknown as string,
                      digest: change.outputState.PackageWrite[1],
                  }
                : {
                      objectId,
                      version: Number(effects.V1.lamportVersion) as unknown as string,
                      digest: change.outputState.ObjectWrite![0],
                  },
            change.outputState.ObjectWrite
                ? change.outputState.ObjectWrite[1]
                : { $kind: 'Immutable', Immutable: true },
        ]);
    const unwrapped = effects.V1.changedObjects
        .filter(
            ([_id, change]) =>
                change.inputState.NotExist &&
                change.outputState.ObjectWrite &&
                change.idOperation.None,
        )
        .map(([objectId, change]) => [
            {
                objectId,
                version: Number(effects.V1.lamportVersion) as unknown as string,
                digest: change.outputState.ObjectWrite![0],
            },
            change.outputState.ObjectWrite![1],
        ]);
    const deleted = effects.V1.changedObjects
        .filter(
            ([_id, change]) =>
                change.inputState.Exist &&
                change.outputState.NotExist &&
                change.idOperation.Deleted,
        )
        .map(([objectId, _change]) => ({
            objectId,
            version: Number(effects.V1.lamportVersion) as unknown as string,
            digest: OBJECT_DIGEST_DELETED,
        }));
    const unwrappedThenDeleted = effects.V1.changedObjects
        .filter(
            ([_id, change]) =>
                change.inputState.NotExist &&
                change.outputState.NotExist &&
                change.idOperation.Deleted,
        )
        .map(([objectId, _change]) => ({
            objectId,
            version: Number(effects.V1.lamportVersion) as unknown as string,
            digest: OBJECT_DIGEST_DELETED,
        }));

    const wrapped = effects.V1.changedObjects
        .filter(
            ([_id, change]) =>
                change.inputState.Exist && change.outputState.NotExist && change.idOperation.None,
        )
        .map(([objectId, _change]) => ({
            objectId,
            version: Number(effects.V1.lamportVersion) as unknown as string,
            digest: OBJECT_DIGEST_WRAPPED,
        }));

    const gasObjectFromV1 =
        effects.V1.gasObjectIndex != null
            ? effects.V1.changedObjects[effects.V1.gasObjectIndex]
            : null;

    const gasObject = gasObjectFromV1
        ? [
              {
                  objectId: gasObjectFromV1[0],
                  digest: gasObjectFromV1[1].outputState.ObjectWrite![0],
                  version: Number(effects.V1.lamportVersion) as unknown as string,
              },
              gasObjectFromV1[1].outputState.ObjectWrite![1],
          ]
        : [
              {
                  objectId: ADDRESS_ZERO,
                  version: '0',
                  digest: OBJECT_DIGEST_ZERO,
              },
              {
                  $kind: 'AddressOwner',
                  AddressOwner: ADDRESS_ZERO,
              },
          ];

    return {
        messageVersion: 'v1',
        status: effects.V1.status.Success
            ? {
                  status: 'success',
              }
            : {
                  status: 'failure',
                  // TODO: we don't have the error message from bcs effects
                  error: effects.V1.status.$kind,
              },
        executedEpoch: effects.V1.executedEpoch,
        gasUsed: effects.V1.gasUsed,
        modifiedAtVersions: modifiedAtVersions.map(([objectId, sequenceNumber]) => ({
            objectId,
            sequenceNumber,
        })),
        ...(sharedObjects.length === 0 ? {} : { sharedObjects }),
        transactionDigest: effects.V1.transactionDigest,
        ...(created.length === 0
            ? {}
            : {
                  created: created.map(([reference, owner]) => ({
                      reference,
                      owner: mapEffectsOwner(owner),
                  })),
              }),
        ...(mutated.length === 0
            ? {}
            : {
                  mutated: mutated.map(([reference, owner]) => ({
                      reference,
                      owner: mapEffectsOwner(owner),
                  })),
              }),
        ...(unwrapped.length === 0
            ? {}
            : {
                  unwrapped:
                      unwrapped.length === 0
                          ? undefined
                          : unwrapped.map(([reference, owner]) => ({
                                reference,
                                owner: mapEffectsOwner(owner),
                            })),
              }),
        ...(deleted.length === 0 ? {} : { deleted: deleted }),
        ...(unwrappedThenDeleted.length === 0
            ? {}
            : { unwrappedThenDeleted: unwrappedThenDeleted }),
        ...(wrapped.length === 0 ? {} : { wrapped: wrapped }),
        gasObject: {
            reference: gasObject[0],
            owner: mapEffectsOwner(gasObject[1]),
        },
        ...(effects.V1.eventsDigest ? { eventsDigest: effects.V1.eventsDigest } : {}),
        dependencies: effects.V1.dependencies,
    };

    function mapEffectsOwner(owner: NonNullable<typeof gasObject>[1]) {
        if (owner.Immutable) {
            return 'Immutable';
        } else if (owner.Shared) {
            return { Shared: { initial_shared_version: owner.Shared.initialSharedVersion } };
        } else if (owner.AddressOwner) {
            return { AddressOwner: owner.AddressOwner };
        } else if (owner.ObjectOwner) {
            return { ObjectOwner: owner.ObjectOwner };
        }

        throw new Error(`Unknown owner type: ${owner}`);
    }
}
