// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TypeTagSerializer, type TypeTag } from '@iota/iota-sdk/bcs';
import { type TransactionArgument, type Commands } from '@iota/iota-sdk/transactions/';
import { formatAddress, normalizeIotaAddress, toB64 } from '@iota/iota-sdk/utils';
import { Collapsible } from '@iota/core';
import { TitleSize } from '@iota/apps-ui-kit';

type TransactionType = ReturnType<(typeof Commands)[keyof typeof Commands]>;
type MakeMoveVecTransaction = ReturnType<(typeof Commands)['MakeMoveVec']>;
type PublishTransaction = ReturnType<(typeof Commands)['Publish']>;

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

    if (arg && typeof arg === 'object' && 'kind' in arg) {
        switch (arg.kind) {
            case 'GasCoin':
                return 'GasCoin';
            case 'Input':
                return `Input(${'index' in arg ? arg.index : 'unknown'})`;
            case 'Result':
                return `Result(${'index' in arg ? arg.index : 'unknown'})`;
            case 'NestedResult':
                return `NestedResult(${'index' in arg ? arg.index : 'unknown'}, ${'resultIndex' in arg ? arg.resultIndex : 'unknown'})`;
            default:
                // eslint-disable-next-line no-console
                console.warn('Unexpected command argument type.', arg);
                return null;
        }
    }
    return null;
}

function convertCommandToString({ $kind, ...command }: TransactionType): string {
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
        <Collapsible hideBorder defaultOpen title={command.$kind} titleSize={TitleSize.Small}>
            <div className="flex flex-col gap-y-sm px-md">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                    {convertCommandToString(command)}
                </span>
            </div>
        </Collapsible>
    );
}
