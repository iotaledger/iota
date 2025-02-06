// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TypeTagSerializer, type TypeTag } from '@iota/iota-sdk/bcs';
import { type TransactionArgument, type Commands } from '@iota/iota-sdk/transactions/';
import { formatAddress, normalizeIotaAddress, toB64 } from '@iota/iota-sdk/utils';
import { Collapsible } from '@iota/core';
import { TitleSize } from '@iota/apps-ui-kit';
import { type IotaArgument, type MoveCallIotaTransaction } from '@iota/iota-sdk/client';
import { ErrorBoundary } from '_src/ui/app/components';

type TransactionType = ReturnType<(typeof Commands)[keyof typeof Commands]>;
type MakeMoveVecTransaction = ReturnType<(typeof Commands)['MakeMoveVec']>;
type PublishTransaction = ReturnType<(typeof Commands)['Publish']>;

function ArrayArgument({ data }: { data: TransactionType }): JSX.Element {
    return (
        <>
            {data &&
                Object.entries(data)
                    .map(
                        ([key, value]) =>
                            `${key}: ${convertCommandArgumentToString(
                                value as
                                    | string
                                    | number
                                    | string[]
                                    | number[]
                                    | TransactionArgument
                                    | TransactionArgument[]
                                    | null,
                            )}`,
                    )
                    .join(', ')}
        </>
    );
}

function MoveCall({ data }: TransactionProps<MoveCallIotaTransaction>): JSX.Element {
    const {
        module,
        package: movePackage,
        function: func,
        arguments: args,
        type_arguments: typeArgs,
    } = data;
    return (
        <span className="text-body-md text-neutral-40 dark:text-neutral-60">
            package:{' '}
            <span className="break-all text-primary-30 dark:text-primary-80">
                {formatAddress(normalizeIotaAddress(movePackage))}
            </span>
            , module:{' '}
            <span className="break-all text-primary-30 dark:text-primary-80">{module}</span>,
            function: <span className="break-all text-primary-30 dark:text-primary-80">{func}</span>
            {args && (
                <span className="break-all">, arguments: [{flattenIotaArguments(args!)}]</span>
            )}
            {typeArgs && (
                <span className="break-all">, type_arguments: [{typeArgs.join(', ')}]</span>
            )}
        </span>
    );
}

function convertCommandArgumentToString(
    arg:
        | string
        | number
        | string[]
        | number[]
        | TransactionArgument
        | TransactionArgument[]
        | MakeMoveVecTransaction['MakeMoveVec']['type']
        | PublishTransaction['Publish']['modules'],
): string | null {
    if (!arg) return null;

    if (typeof arg === 'string' || typeof arg === 'number') return String(arg);

    if (typeof arg === 'object' && 'None' in arg) {
        return null;
    }

    if (typeof arg === 'object' && 'Some' in arg) {
        if (typeof arg.Some === 'object') {
            // MakeMoveVecTransaction['type'] is TypeTag type
            return TypeTagSerializer.tagToString(arg.Some as TypeTag);
        }
        return String(arg.Some);
    }

    if (Array.isArray(arg)) {
        // Publish transaction special casing:
        if (typeof arg[0] === 'number') {
            return toB64(new Uint8Array(arg as number[]));
        }

        return `[${arg.map((argVal) => convertCommandArgumentToString(argVal)).join(', ')}]`;
    }
    if (arg && typeof arg === 'object' && '$kind' in arg) {
        switch (arg.$kind) {
            case 'GasCoin':
                return 'GasCoin';
            case 'Input':
                return `Input(${'Input' in arg ? arg.Input : 'unknown'})`;
            case 'Result':
                return `Result(${'Result' in arg ? arg.Result : 'unknown'})`;
            case 'NestedResult':
                return `NestedResult(${'NestedResult' in arg ? arg.NestedResult : 'unknown'}, ${'resultIndex' in arg ? arg.resultIndex : 'unknown'})`;
            default:
                // eslint-disable-next-line no-console
                console.warn('Unexpected command argument type.', arg);
                return null;
        }
    }
    return null;
}

function convertCommandToString({ $kind, ...command }: TransactionType): JSX.Element {
    const [[type, data]] = Object.entries(command);
    if (type === 'MoveCall') {
        return (
            <ErrorBoundary>
                <MoveCall type={type} data={data as MoveCallIotaTransaction} />;
            </ErrorBoundary>
        );
    }
    return (
        <ErrorBoundary>
            <ArrayArgument data={data as TransactionType} />;
        </ErrorBoundary>
    );
}

interface TransactionProps<T> {
    type: string;
    data: T;
}

export function flattenIotaArguments(data: (IotaArgument | IotaArgument[])[]): string {
    if (!data) {
        return '';
    }

    return data
        .map((value) => {
            if (value === 'GasCoin') {
                return value;
            } else if (Array.isArray(value)) {
                return `[${flattenIotaArguments(value)}]`;
            } else if (value === null) {
                return 'Null';
            } else if (typeof value === 'object') {
                if ('Input' in value) {
                    return `Input(${value.Input})`;
                } else if ('Result' in value) {
                    return `Result(${value.Result})`;
                } else if ('NestedResult' in value) {
                    return `NestedResult(${value.NestedResult[0]}, ${value.NestedResult[1]})`;
                }
            } else if (typeof value === 'string') {
                return value;
            } else {
                throw new Error('Not a correct flattenable data');
            }
        })
        .join(', ');
}
interface CommandProps {
    command: TransactionType;
}

export function Command({ command }: CommandProps) {
    return (
        <Collapsible hideBorder defaultOpen title={command.$kind} titleSize={TitleSize.Small}>
            <div className="flex flex-col gap-y-sm px-md">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                    {convertCommandToString(command)}
                </span>
            </div>
        </Collapsible>
    );
}
