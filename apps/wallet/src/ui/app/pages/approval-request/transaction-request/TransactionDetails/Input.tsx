// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ExplorerLink from '_src/ui/app/components/explorer-link';
import { ExplorerLinkType } from '_src/ui/app/components/explorer-link/ExplorerLinkType';
import { Text } from '_src/ui/app/shared/text';
import { type TransactionBlockInput } from '@iota/iota-sdk/transactions';
import { formatAddress, toB64 } from '@iota/iota-sdk/utils';

interface InputProps {
    input: TransactionBlockInput;
}

export function Input({ input }: InputProps) {
    const { objectId } = input.value?.Object?.ImmOrOwned || input.value?.Object?.Shared || {};

    return (
        <div className="break-all">
            <Text variant="pBodySmall" weight="medium" color="steel-dark" mono>
                {'Pure' in input.value ? (
                    `${toB64(new Uint8Array(input.value.Pure))}`
                ) : 'Object' in input.value ? (
                    <ExplorerLink type={ExplorerLinkType.Object} objectID={objectId}>
                        {formatAddress(objectId)}
                    </ExplorerLink>
                ) : (
                    'Unknown input value'
                )}
            </Text>
        </div>
    );
}
