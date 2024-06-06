// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type MoveCallIotaTransaction,
    type IotaArgument,
    type IotaMovePackage,
} from '@iota/iota.js/client';
import { Text } from '@iota/ui';
import { type ReactNode } from 'react';

import { flattenIotaArguments } from './utils';
import { ErrorBoundary } from '~/components/error-boundary/ErrorBoundary';
import { ObjectLink } from '~/ui/InternalLink';

export interface TransactionProps<T> {
    type: string;
    data: T;
}

function TransactionContent({ children }: { children?: ReactNode }) {
    return (
        <Text variant="pBody/normal" color="steel-dark">
            {children}
        </Text>
    );
}

function ArrayArgument({ data }: TransactionProps<(IotaArgument | IotaArgument[])[] | undefined>) {
    return (
        <TransactionContent>
            {data && (
                <span className="break-all">
                    <Text variant="pBody/medium">({flattenIotaArguments(data)})</Text>
                </span>
            )}
        </TransactionContent>
    );
}

function MoveCall({ data }: TransactionProps<MoveCallIotaTransaction>) {
    const {
        module,
        package: movePackage,
        function: func,
        arguments: args,
        type_arguments: typeArgs,
    } = data;

    return (
        <TransactionContent>
            <Text variant="pBody/medium">
                (package: <ObjectLink objectId={movePackage} />, module:{' '}
                <ObjectLink objectId={`${movePackage}?module=${module}`} label={`'${module}'`} />,
                function: <span className="break-all text-hero-dark">{func}</span>
                {args && (
                    <span className="break-all">, arguments: [{flattenIotaArguments(args!)}]</span>
                )}
                {typeArgs && (
                    <span className="break-all">, type_arguments: [{typeArgs.join(', ')}]</span>
                )}
            </Text>
        </TransactionContent>
    );
}

export function Transaction({
    type,
    data,
}: TransactionProps<
    (IotaArgument | IotaArgument[])[] | MoveCallIotaTransaction | IotaMovePackage
>) {
    if (type === 'MoveCall') {
        return (
            <ErrorBoundary>
                <MoveCall type={type} data={data as MoveCallIotaTransaction} />
            </ErrorBoundary>
        );
    }

    return (
        <ErrorBoundary>
            <ArrayArgument
                type={type}
                data={type !== 'Publish' ? (data as (IotaArgument | IotaArgument[])[]) : undefined}
            />
        </ErrorBoundary>
    );
}
