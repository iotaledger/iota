// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy } from '@iota/apps-ui-icons';
import { ButtonUnstyled } from '@iota/apps-ui-kit';
import { AddressAlias } from '@iota/core';
import { formatAddress, formatDigest, formatType } from '@iota/iota-sdk/utils';
import React, { type ReactNode } from 'react';

import { Link, type LinkProps } from '~/components/ui';
import { onCopySuccess } from '~/lib';

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
        ...props
    }: BaseInternalLinkProps & Record<T, string>) => {
        const truncatedAddress = noTruncate ? id : formatter(id);
        const queryString = new URLSearchParams(queryStrings).toString();
        const queryStringPrefix = queryString ? `?${queryString}` : '';

        const to = `/${base}/${encodeURI(id)}${queryStringPrefix}`;

        async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement>) {
            event.stopPropagation();
            if (!navigator.clipboard) {
                return;
            }
            if (copyText) {
                try {
                    await navigator.clipboard.writeText(copyText);
                    onCopySuccess();
                } catch (error) {
                    console.error('Failed to copy:', error);
                    onCopyError?.(error, copyText);
                }
            }
        }

        if (showAddressAlias) {
            return (
                <AddressAlias
                    address={id}
                    noFormatAddress={noTruncate}
                    onCopy={copyText ? handleCopyClick : undefined}
                    renderAddress={(address) => (
                        <Link
                            className="text-iota-primary-30 dark:text-iota-primary-80"
                            variant="mono"
                            to={to}
                            {...props}
                        >
                            {label || address}
                        </Link>
                    )}
                    renderAlias={renderAddressAlias}
                />
            );
        }

        return (
            <div className="flex flex-row items-center gap-x-xxs">
                <Link
                    className="text-iota-primary-30 dark:text-iota-primary-80"
                    variant="mono"
                    to={to}
                    {...props}
                >
                    {label || truncatedAddress}
                </Link>
                {copyText && (
                    <ButtonUnstyled onClick={handleCopyClick}>
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
export const AddressLink = createInternalLink('address', 'address', (addressOrNs) =>
    formatAddress(addressOrNs),
);
export const ObjectLink = createInternalLink('object', 'objectId', formatType);
export const TransactionLink = createInternalLink('txblock', 'digest', formatDigest);
export const ValidatorLink = createInternalLink('validator', 'address', formatAddress);
