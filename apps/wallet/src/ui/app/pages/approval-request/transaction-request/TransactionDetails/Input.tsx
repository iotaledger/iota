// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLinkType } from '_components';
import { type TransactionBlockInput } from '@iota/iota-sdk/transactions';
import { formatAddress, toB64 } from '@iota/iota-sdk/utils';
import { useExplorerLink } from '_src/ui/app/hooks/useExplorerLink';
import { KeyValueInfo } from '@iota/apps-ui-kit';

interface InputProps {
    input: TransactionBlockInput;
}

export function Input({ input }: InputProps) {
    const { objectId } = input.value?.Object?.ImmOrOwned || input.value?.Object?.Shared || {};
    const objectIdLink = useExplorerLink({
        type: ExplorerLinkType.Object,
        objectID: objectId || '',
    });
    return (
        <div className="flex flex-col gap-y-sm px-md">
            {'Pure' in input.value ? (
                <KeyValueInfo
                    keyText="Pure"
                    valueText={toB64(new Uint8Array(input.value.Pure))}
                    fullwidth
                />
            ) : 'Object' in input.value ? (
                <KeyValueInfo
                    keyText="Object"
                    valueText={formatAddress(objectId)}
                    valueLink={objectIdLink || ''}
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
