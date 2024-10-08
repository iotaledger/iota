// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TypeTagSerializer, type TypeTag } from '@iota/iota-sdk/bcs';
import { type TransactionArgument, type Transactions } from '@iota/iota-sdk/transactions';
import { formatAddress, normalizeIotaAddress, toB64 } from '@iota/iota-sdk/utils';
import { Collapsible } from '_src/ui/app/shared/collapse';
import { TitleSize } from '@iota/apps-ui-kit';

type TransactionType = ReturnType<(typeof Transactions)[keyof typeof Transactions]>;
type MakeMoveVecTransaction = ReturnType<(typeof Transactions)['MakeMoveVec']>;
type PublishTransaction = ReturnType<(typeof Transactions)['Publish']>;

function convertCommandArgumentToString(
    arg:
        | string
        | number
        | string[]
        | number[]
        | TransactionArgument
        | TransactionArgument[]
        | MakeMoveVecTransaction['type']
        | PublishTransaction['modules'],
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
        return arg.Some;
    }

    if (Array.isArray(arg)) {
        // Publish transaction special casing:
        if (typeof arg[0] === 'number') {
            return toB64(new Uint8Array(arg as number[]));
        }

        return `[${arg.map((argVal) => convertCommandArgumentToString(argVal)).join(', ')}]`;
    }

    switch (arg.kind) {
        case 'GasCoin':
            return 'GasCoin';
        case 'Input':
            return `Input(${arg.index})`;
        case 'Result':
            return `Result(${arg.index})`;
        case 'NestedResult':
            return `NestedResult(${arg.index}, ${arg.resultIndex})`;
        default:
            // eslint-disable-next-line no-console
            console.warn('Unexpected command argument type.', arg);
            return null;
    }
}

function convertCommandToString({ kind, ...command }: TransactionType): string {
    const commandArguments = Object.entries(command);

    return commandArguments
        .map(([key, value]) => {
            if (key === 'target') {
                const [packageId, moduleName, functionName] = value.split('::');
                return [
                    `package: ${formatAddress(normalizeIotaAddress(packageId))}`,
                    `module: ${moduleName}`,
                    `function: ${functionName}`,
                ].join(', ');
            }

            const stringValue = convertCommandArgumentToString(value);

            if (!stringValue) return null;

            return `${key}: ${stringValue}`;
        })
        .filter(Boolean)
        .join(', ');
}

interface CommandProps {
    command: TransactionType;
}

export function Command({ command }: CommandProps) {
    return (
        <Collapsible hideBorder defaultOpen title={command.kind} titleSize={TitleSize.Small}>
            <div className="flex flex-col gap-y-sm px-md">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                    {convertCommandToString(command)}
                </span>
            </div>
        </Collapsible>
    );
}
