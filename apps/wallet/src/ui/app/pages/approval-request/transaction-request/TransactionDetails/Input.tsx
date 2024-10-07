// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType } from '_components';
import { type TransactionInput } from '@iota/iota-sdk/transactions';
import { formatAddress, toB64 } from '@iota/iota-sdk/utils';
import { KeyValueInfo } from '@iota/apps-ui-kit';

interface InputProps {
    input: TransactionInput;
}

export function Input({ input }: InputProps) {
    const { objectId } = input?.Object?.ImmOrOwnedObject || input?.Object?.SharedObject || {};

    return (
        <div className="flex flex-col gap-y-sm px-md">
            {'Pure' in input ? (
                <KeyValueInfo
                    keyText="Pure"
                    value={toB64(new Uint8Array(Buffer.from(input.Pure?.bytes || [])))}
                    fullwidth
                />
            ) : 'Object' in input ? (
                <KeyValueInfo
                    keyText="Object"
                    value={
                        <ExplorerLink type={ExplorerLinkType.Object} objectID={objectId || ''}>
                            {formatAddress(objectId || '')}
                        </ExplorerLink>
                    }
                    fullwidth
                />
            ) : (
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                    Unknown input value
                </span>
            )}
        </div>
    );
}
