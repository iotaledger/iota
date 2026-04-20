// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy } from '@iota/apps-ui-icons';
import { ButtonUnstyled } from '@iota/apps-ui-kit';
import {
    NamedAddressTooltip,
    AddressAlias,
    useGetDefaultIotaName,
    useCopyToClipboard,
} from '@iota/core';
import { isValidIotaName } from '@iota/iota-names-sdk';
import { formatAddress, formatDigest, formatType, isValidIotaAddress } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import React, { type ReactNode } from 'react';

import { Link, type LinkProps } from '~/components/ui';

interface BaseInternalLinkProps extends LinkProps {
    showAddressAlias?: boolean;
    noTruncate?: boolean;
    label?: string | ReactNode;
    renderAddressAlias?: (alias: string) => ReactNode;
    queryStrings?: Record<string, string>;
    copyText?: string;
    onCopyError?: (e: unknown, text: string) => void;
}

function createInternalLink<T extends string>(
    base: string,
    propName: T,
    formatter: (id: string) => string = (id) => id,
): (props: BaseInternalLinkProps & Record<T, string>) => JSX.Element {
    return ({
        [propName]: id,
        noTruncate,
        label,
        queryStrings = {},
        copyText,
        onCopyError,
        renderAddressAlias,
        showAddressAlias = ['address', 'object', 'validator'].includes(base),
        className,
        ...props
    }: BaseInternalLinkProps & Record<T, string>) => {
        const truncatedAddress = noTruncate ? id : formatter(id);
        const queryString = new URLSearchParams(queryStrings).toString();
        const queryStringPrefix = queryString ? `?${queryString}` : '';

        const to = `/${base}/${encodeURI(id)}${queryStringPrefix}`;

        const isResolveIotaName = base === 'address' && isValidIotaAddress(id);
        const { data: iotaName } = useGetDefaultIotaName(isResolveIotaName ? id : null);
        const copyToClipboard = useCopyToClipboard();

        async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement>) {
            event.stopPropagation();
            if (copyText) {
                const success = await copyToClipboard(copyText);
                if (!success && onCopyError) {
                    onCopyError(new Error('Clipboard write failed'), copyText);
                }
            }
        }

        if (showAddressAlias) {
            return (
                <AddressAlias
                    address={id}
                    onCopy={copyText ? handleCopyClick : undefined}
                    noTruncate={noTruncate}
                    truncateUnknown={!noTruncate}
                    renderAddress={(address) => (
                        <NamedAddressTooltip name={iotaName} address={address}>
                            <Link
                                className={clsx(
                                    'text-iota-primary-30 dark:text-iota-primary-80',
                                    className,
                                )}
                                variant="mono"
                                to={to}
                                {...props}
                            >
                                {iotaName || label || address}
                            </Link>
                        </NamedAddressTooltip>
                    )}
                    renderAlias={renderAddressAlias}
                />
            );
        }

        return (
            <div className="flex flex-row items-center gap-x-xxs">
                <Link
                    className={clsx('text-iota-primary-30 dark:text-iota-primary-80', className)}
                    variant="mono"
                    to={to}
                    {...props}
                >
                    {label || truncatedAddress}
                </Link>
                {copyText && (
                    <ButtonUnstyled onClick={handleCopyClick} aria-label="Copy to clipboard">
                        <Copy className="text-iota-neutral-60 dark:text-iota-neutral-40" />
                    </ButtonUnstyled>
                )}
            </div>
        );
    };
}

export const EpochLink = createInternalLink('epoch', 'epoch');
export const CheckpointLink = createInternalLink('checkpoint', 'digest', formatAddress);
export const CheckpointSequenceLink = createInternalLink('checkpoint', 'sequence');
export const AddressLink = createInternalLink('address', 'address', (addressOrName) => {
    if (isValidIotaName(addressOrName)) {
        return addressOrName;
    }

    return formatAddress(addressOrName);
});
export const ObjectLink = createInternalLink('object', 'objectId', formatType);
export const TransactionLink = createInternalLink('txblock', 'digest', formatDigest);
export const ValidatorLink = createInternalLink('validator', 'address', formatAddress);
