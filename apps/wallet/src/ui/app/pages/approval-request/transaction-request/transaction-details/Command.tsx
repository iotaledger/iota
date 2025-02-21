// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TypeTagSerializer, type TypeTag } from '@iota/iota-sdk/bcs';
import { type Commands } from '@iota/iota-sdk/transactions/';
import { formatAddress, normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { Collapsible } from '@iota/core';
import { TitleSize } from '@iota/apps-ui-kit';
import { type IotaArgument, type MoveCallIotaTransaction } from '@iota/iota-sdk/client';
import { ErrorBoundary } from '_src/ui/app/components';

type TransactionType = ReturnType<(typeof Commands)[keyof typeof Commands]>;
type CommandArgTypes = string | string[] | IotaArgument | IotaArgument[] | null;

function CommandArgument({ data }: { data: TransactionType }): JSX.Element {
    return (
        <>
            {data &&
                Object.entries(data)
                    .map(
                        ([key, value]) =>
                            `${key}: ${convertCommandArgumentToString(value as CommandArgTypes)}`,
                    )
                    .join(', ')}
        </>
    );
}

function MoveCall({ data }: { data: MoveCallIotaTransaction }): JSX.Element {
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
                <span className="break-all">
                    , arguments: {convertCommandArgumentToString(args)}
                </span>
            )}
            {typeArgs && (
                <span className="break-all">, type_arguments: [{typeArgs.join(', ')}]</span>
            )}
        </span>
    );
}

function convertCommandArgumentToString(arg: CommandArgTypes): string | null {
    if (!arg) return null;

    if (typeof arg === 'string' || typeof arg === 'number') return String(arg);

    if (typeof arg === 'object' && 'None' in arg) {
        return null;
    }

    if (typeof arg === 'object' && 'Some' in arg) {
        if (typeof arg.Some === 'object') {
            return TypeTagSerializer.tagToString(arg.Some as TypeTag);
        }
        return String(arg.Some);
    }

    if (Array.isArray(arg)) {
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
                return `NestedResult(${'NestedResult' in arg ? `${arg.NestedResult[0]}, ${arg.NestedResult[1]})` : 'unknown'}`;
            default:
                // eslint-disable-next-line no-console
                console.warn('Unexpected command argument type.', arg);
                return null;
        }
    }
    return null;
}

interface CommandProps {
    command: TransactionType;
}

export function Command({ command }: CommandProps) {
    const [[type, data]] = Object.entries(command);
    return (
        <Collapsible hideBorder defaultOpen title={command.$kind} titleSize={TitleSize.Small}>
            <div className="flex flex-col gap-y-sm px-md">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                    {type === 'MoveCall' ? (
                        <ErrorBoundary>
                            <MoveCall data={data as MoveCallIotaTransaction} />
                        </ErrorBoundary>
                    ) : (
                        <ErrorBoundary>
                            <CommandArgument data={data as TransactionType} />
                        </ErrorBoundary>
                    )}
                </span>
            </div>
        </Collapsible>
    );
}
