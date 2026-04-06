// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType } from '_components';
import { type IotaCallArg } from '@iota/iota-sdk/client';
import { type TransactionInput } from '@iota/iota-sdk/transactions';
import { formatAddress, toBase64 } from '@iota/iota-sdk/utils';
import { KeyValueInfo } from '@iota/apps-ui-kit';
import type { ReactNode } from 'react';
import { formatPureInputValue, getPureValueTypeLabel } from './pureValueType';

interface PureInputProps {
    input: TransactionInput;
    dryRunInput?: IotaCallArg;
}

function PureInput({ input, dryRunInput }: PureInputProps) {
    if (dryRunInput?.type !== 'pure') {
        const bytes = 'Pure' in input ? input.Pure?.bytes || [] : [];
        return (
            <KeyValueInfo
                keyText="Pure"
                value={toBase64(new Uint8Array(Buffer.from(bytes)))}
                fullwidth
            />
        );
    }

    const keyText = getPureValueTypeLabel(dryRunInput.valueType);

    let value: ReactNode;
    if (dryRunInput.valueType === 'address') {
        const addr = String(dryRunInput.value);
        value = (
            <ExplorerLink type={ExplorerLinkType.Address} address={addr} eventType="address">
                {formatAddress(addr)}
            </ExplorerLink>
        );
    } else {
        value = formatPureInputValue(dryRunInput.value, dryRunInput.valueType);
    }

    return <KeyValueInfo keyText={keyText} value={value} fullwidth />;
}

interface InputProps {
    input: TransactionInput;
    dryRunInput?: IotaCallArg;
}

export function Input({ input, dryRunInput }: InputProps) {
    const { objectId } = input?.Object?.ImmOrOwnedObject || input?.Object?.SharedObject || {};

    return (
        <div className="flex flex-col gap-y-sm px-md">
            {'Pure' in input ? (
                <PureInput input={input} dryRunInput={dryRunInput} />
            ) : 'Object' in input ? (
                <KeyValueInfo
                    keyText="Object"
                    value={
                        <ExplorerLink
                            type={ExplorerLinkType.Object}
                            objectID={objectId || ''}
                            eventType="object"
                        >
                            <span data-amp-mask>{formatAddress(objectId || '')}</span>
                        </ExplorerLink>
                    }
                    fullwidth
                />
            ) : (
                <span className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                    Unknown input value
                </span>
            )}
        </div>
    );
}
