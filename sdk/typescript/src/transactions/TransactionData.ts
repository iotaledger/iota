// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toBase58 } from '@iota/bcs';
import type { InferInput } from 'valibot';
import { parse } from 'valibot';

import { bcs } from '../bcs/index.js';
import { normalizeIotaAddress } from '../utils/iota-types.js';
import type {
    Argument,
    CallArg,
    Command,
    GasData,
    TransactionExpiration,
} from './data/internal.js';
import { Argument as ArgumentSchema, TransactionData } from './data/internal.js';
import { transactionDataFromV1 } from './data/v1.js';
import type { SerializedTransactionDataV1 } from './data/v1.js';
import type { SerializedTransactionDataV2 } from './data/v2.js';
import { hashTypedData } from './hash.js';
import { getIdFromCallArg, remapCommandArguments } from './utils.js';
import type { TransactionResult } from './Transaction.js';

function prepareIotaAddress(address: string) {
    return normalizeIotaAddress(address).replace('0x', '');
}

export class TransactionDataBuilder implements TransactionData {
    static fromKindBytes(bytes: Uint8Array) {
        const kind = bcs.TransactionKind.parse(bytes);

        const programmableTx = kind.ProgrammableTransaction;
        if (!programmableTx) {
            throw new Error('Unable to deserialize from bytes.');
        }

        return TransactionDataBuilder.restore({
            version: 2,
            sender: null,
            expiration: null,
            gasData: {
                budget: null,
                owner: null,
                payment: null,
                price: null,
            },
            inputs: programmableTx.inputs,
            commands: programmableTx.commands,
        });
    }

    static fromBytes(bytes: Uint8Array) {
        const rawData = bcs.TransactionData.parse(bytes);
        const data = rawData?.V1;
        const programmableTx = data.kind.ProgrammableTransaction;

        if (!data || !programmableTx) {
            throw new Error('Unable to deserialize from bytes.');
        }

        return TransactionDataBuilder.restore({
            version: 2,
            sender: data.sender,
            expiration: data.expiration,
            gasData: data.gasData,
            inputs: programmableTx.inputs,
            commands: programmableTx.commands,
        });
    }

    static restore(
        data:
            | InferInput<typeof SerializedTransactionDataV2>
            | InferInput<typeof SerializedTransactionDataV1>,
    ) {
        if (data.version === 2) {
            return new TransactionDataBuilder(parse(TransactionData, data));
        } else {
            return new TransactionDataBuilder(parse(TransactionData, transactionDataFromV1(data)));
        }
    }

    /**
     * Generate transaction digest.
     *
     * @param bytes BCS serialized transaction data
     * @returns transaction digest.
     */
    static getDigestFromBytes(bytes: Uint8Array) {
        const hash = hashTypedData('TransactionData', bytes);
        return toBase58(hash);
    }

    version = 2 as const;
    sender: string | null;
    expiration: TransactionExpiration | null;
    gasData: GasData;
    inputs: CallArg[];
    commands: Command[];

    constructor(clone?: TransactionData) {
        this.sender = clone?.sender ?? null;
        this.expiration = clone?.expiration ?? null;
        this.inputs = clone?.inputs ?? [];
        this.commands = clone?.commands ?? [];
        this.gasData = clone?.gasData ?? {
            budget: null,
            price: null,
            owner: null,
            payment: null,
        };
    }

    build({
        maxSizeBytes = Infinity,
        overrides,
        onlyTransactionKind,
    }: {
        maxSizeBytes?: number;
        overrides?: {
            expiration?: TransactionExpiration;
            sender?: string;
            // @deprecated use gasData instead
            gasConfig?: Partial<GasData>;
            gasData?: Partial<GasData>;
        };
        onlyTransactionKind?: boolean;
    } = {}) {
        // TODO validate that inputs and intents are actually resolved
        const inputs = this.inputs as (typeof bcs.CallArg.$inferInput)[];
        const commands = this.commands as Extract<
            Command<Exclude<Argument, { IntentResult: unknown } | { NestedIntentResult: unknown }>>,
            { Upgrade: unknown }
        >[];

        const kind = {
            ProgrammableTransaction: {
                inputs,
                commands,
            },
        };

        if (onlyTransactionKind) {
            return bcs.TransactionKind.serialize(kind, { maxSize: maxSizeBytes }).toBytes();
        }

        const expiration = overrides?.expiration ?? this.expiration;
        const sender = overrides?.sender ?? this.sender;
        const gasData = { ...this.gasData, ...overrides?.gasConfig, ...overrides?.gasData };

        if (!sender) {
            throw new Error('Missing transaction sender');
        }

        if (!gasData.budget) {
            throw new Error('Missing gas budget');
        }

        if (!gasData.payment) {
            throw new Error('Missing gas payment');
        }

        if (!gasData.price) {
            throw new Error('Missing gas price');
        }

        const transactionData = {
            sender: prepareIotaAddress(sender),
            expiration: expiration ? expiration : { None: true },
            gasData: {
                payment: gasData.payment,
                owner: prepareIotaAddress(this.gasData.owner ?? sender),
                price: BigInt(gasData.price),
                budget: BigInt(gasData.budget),
            },
            kind: {
                ProgrammableTransaction: {
                    inputs,
                    commands,
                },
            },
        };

        return bcs.TransactionData.serialize(
            { V1: transactionData },
            { maxSize: maxSizeBytes },
        ).toBytes();
    }

    addInput<T extends 'object' | 'pure'>(type: T, arg: CallArg) {
        const index = this.inputs.length;
        this.inputs.push(arg);
        return { Input: index, type, $kind: 'Input' as const };
    }

    getInputUses(index: number, fn: (arg: Argument, command: Command) => void) {
        this.mapArguments((arg, command) => {
            if (arg.$kind === 'Input' && arg.Input === index) {
                fn(arg, command);
            }

            return arg;
        });
    }

    mapCommandArguments(
        index: number,
        fn: (arg: Argument, command: Command, commandIndex: number) => Argument,
    ) {
        const command = this.commands[index];

        switch (command.$kind) {
            case 'MoveCall':
                command.MoveCall.arguments = command.MoveCall.arguments.map((arg) =>
                    fn(arg, command, index),
                );
                break;
            case 'TransferObjects':
                command.TransferObjects.objects = command.TransferObjects.objects.map((arg) =>
                    fn(arg, command, index),
                );
                command.TransferObjects.address = fn(
                    command.TransferObjects.address,
                    command,
                    index,
                );
                break;
            case 'SplitCoins':
                command.SplitCoins.coin = fn(command.SplitCoins.coin, command, index);
                command.SplitCoins.amounts = command.SplitCoins.amounts.map((arg) =>
                    fn(arg, command, index),
                );
                break;
            case 'MergeCoins':
                command.MergeCoins.destination = fn(command.MergeCoins.destination, command, index);
                command.MergeCoins.sources = command.MergeCoins.sources.map((arg) =>
                    fn(arg, command, index),
                );
                break;
            case 'MakeMoveVec':
                command.MakeMoveVec.elements = command.MakeMoveVec.elements.map((arg) =>
                    fn(arg, command, index),
                );
                break;
            case 'Upgrade':
                command.Upgrade.ticket = fn(command.Upgrade.ticket, command, index);
                break;
            case '$Intent':
                const inputs = command.$Intent.inputs;
                command.$Intent.inputs = {};

                for (const [key, value] of Object.entries(inputs)) {
                    command.$Intent.inputs[key] = Array.isArray(value)
                        ? value.map((arg) => fn(arg, command, index))
                        : fn(value, command, index);
                }

                break;
            case 'Publish':
                break;
            default:
                throw new Error(
                    `Unexpected transaction kind: ${(command as { $kind: unknown }).$kind}`,
                );
        }
    }

    mapArguments(fn: (arg: Argument, command: Command, commandIndex: number) => Argument) {
        for (const commandIndex of this.commands.keys()) {
            this.mapCommandArguments(commandIndex, fn);
        }
    }

    replaceCommand(
        index: number,
        replacement: Command | Command[],
        resultIndex: number | { Result: number } | { NestedResult: [number, number] } = index,
    ) {
        if (!Array.isArray(replacement)) {
            this.commands[index] = replacement;
            return;
        }

        const sizeDiff = replacement.length - 1;

        this.commands.splice(index, 1, ...structuredClone(replacement));

        this.mapArguments((arg, _command, commandIndex) => {
            if (commandIndex < index + replacement.length) {
                return arg;
            }

            if (typeof resultIndex !== 'number') {
                if (
                    (arg.$kind === 'Result' && arg.Result === index) ||
                    (arg.$kind === 'NestedResult' && arg.NestedResult[0] === index)
                ) {
                    if (!('NestedResult' in arg) || arg.NestedResult[1] === 0) {
                        return parse(ArgumentSchema, structuredClone(resultIndex));
                    } else {
                        throw new Error(
                            `Cannot replace command ${index} with a specific result type: NestedResult[${index}, ${arg.NestedResult[1]}] references a nested element that cannot be mapped to the replacement result`,
                        );
                    }
                }
            }

            // Handle adjustment of other references
            switch (arg.$kind) {
                case 'Result':
                    if (arg.Result === index && typeof resultIndex === 'number') {
                        arg.Result = resultIndex;
                    }
                    if (arg.Result > index) {
                        arg.Result += sizeDiff;
                    }
                    break;

                case 'NestedResult':
                    if (arg.NestedResult[0] === index && typeof resultIndex === 'number') {
                        return {
                            $kind: 'NestedResult',
                            NestedResult: [resultIndex, arg.NestedResult[1]],
                        };
                    }
                    if (arg.NestedResult[0] > index) {
                        arg.NestedResult[0] += sizeDiff;
                    }
                    break;
            }
            return arg;
        });
    }

    replaceCommandWithTransaction(
        index: number,
        otherTransaction: TransactionData,
        result: TransactionResult,
    ) {
        if (result.$kind !== 'Result' && result.$kind !== 'NestedResult') {
            throw new Error('Result must be of kind Result or NestedResult');
        }

        this.insertTransaction(index, otherTransaction);

        this.replaceCommand(
            index + otherTransaction.commands.length,
            [],
            'Result' in result
                ? { NestedResult: [result.Result + index, 0] }
                : {
                      NestedResult: [
                          (result as { NestedResult: [number, number] }).NestedResult[0] + index,
                          (result as { NestedResult: [number, number] }).NestedResult[1],
                      ] as [number, number],
                  },
        );
    }

    insertTransaction(atCommandIndex: number, otherTransaction: TransactionData) {
        const inputMapping = new Map<number, number>();
        const commandMapping = new Map<number, number>();

        for (let i = 0; i < otherTransaction.inputs.length; i++) {
            const otherInput = otherTransaction.inputs[i];
            const id = getIdFromCallArg(otherInput);

            let existingIndex = -1;
            if (id !== undefined) {
                existingIndex = this.inputs.findIndex((input) => getIdFromCallArg(input) === id);

                if (
                    existingIndex !== -1 &&
                    this.inputs[existingIndex].Object?.SharedObject &&
                    otherInput.Object?.SharedObject
                ) {
                    this.inputs[existingIndex].Object!.SharedObject!.mutable =
                        this.inputs[existingIndex].Object!.SharedObject!.mutable ||
                        otherInput.Object.SharedObject.mutable;
                }
            }

            if (existingIndex !== -1) {
                inputMapping.set(i, existingIndex);
            } else {
                const newIndex = this.inputs.length;
                this.inputs.push(otherInput);
                inputMapping.set(i, newIndex);
            }
        }

        for (let i = 0; i < otherTransaction.commands.length; i++) {
            commandMapping.set(i, atCommandIndex + i);
        }

        const remappedCommands: Command[] = [];
        for (let i = 0; i < otherTransaction.commands.length; i++) {
            const command = structuredClone(otherTransaction.commands[i]);

            remapCommandArguments(command, inputMapping, commandMapping);

            remappedCommands.push(command);
        }

        this.commands.splice(atCommandIndex, 0, ...remappedCommands);

        const sizeDiff = remappedCommands.length;
        if (sizeDiff > 0) {
            this.mapArguments((arg, _command, commandIndex) => {
                if (
                    commandIndex >= atCommandIndex &&
                    commandIndex < atCommandIndex + remappedCommands.length
                ) {
                    return arg;
                }

                switch (arg.$kind) {
                    case 'Result':
                        if (arg.Result >= atCommandIndex) {
                            arg.Result += sizeDiff;
                        }
                        break;

                    case 'NestedResult':
                        if (arg.NestedResult[0] >= atCommandIndex) {
                            arg.NestedResult[0] += sizeDiff;
                        }
                        break;
                }
                return arg;
            });
        }
    }

    getDigest() {
        const bytes = this.build({ onlyTransactionKind: false });
        return TransactionDataBuilder.getDigestFromBytes(bytes);
    }

    snapshot(): TransactionData {
        return parse(TransactionData, this);
    }
}
