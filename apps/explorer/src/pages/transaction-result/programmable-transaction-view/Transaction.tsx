// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type MoveCallIotaTransaction,
    type IotaArgument,
    type IotaMovePackage,
} from '@iota/iota-sdk/client';
import { flattenIotaArguments } from './utils';
import { ErrorBoundary } from '~/components';
import { ObjectLink } from '~/components/ui';
import { onCopySuccess } from '~/lib/utils';

interface TransactionProps<T> {
    type: string;
    data: T;
}

function ArrayArgument({
    data,
}: TransactionProps<(IotaArgument | IotaArgument[])[] | undefined>): JSX.Element {
    return (
        <>
            {data && (
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                    ({flattenIotaArguments(data)})
                </span>
            )}
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
            <ObjectLink
                objectId={movePackage}
                copyText={movePackage}
                onCopySuccess={onCopySuccess}
            />
            , module:{' '}
            <ObjectLink objectId={`${movePackage}?module=${module}`} label={`'${module}'`} />,
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

export function Transaction({
    type,
    data,
}: TransactionProps<
    (IotaArgument | IotaArgument[])[] | MoveCallIotaTransaction | IotaMovePackage
>): JSX.Element {
    if (type === 'MoveCall') {
        return (
            <ErrorBoundary>
                <MoveCall type={type} data={data as MoveCallIotaTransaction} />
            </ErrorBoundary>
        );
    }

    return (
        <ErrorBoundary>
            <ArrayArgument type={type} data={data as (IotaArgument | IotaArgument[])[]} />
        </ErrorBoundary>
    );
}
