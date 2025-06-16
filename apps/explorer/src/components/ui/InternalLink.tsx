// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy } from '@iota/apps-ui-icons';
import { ButtonUnstyled } from '@iota/apps-ui-kit';
import { formatAddress, formatDigest, formatType } from '@iota/iota-sdk/utils';
import { type ReactNode } from 'react';

import { Link, type LinkProps } from '~/components/ui';

interface BaseInternalLinkProps extends LinkProps {
    noTruncate?: boolean;
    label?: string | ReactNode;
    queryStrings?: Record<string, string>;
    copyText?: string;
    onCopySuccess?: (e: React.MouseEvent<HTMLButtonElement>, text: string) => void;
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
        onCopySuccess,
        onCopyError,
        ...props
    }: BaseInternalLinkProps & Record<T, string>) => {
        const truncatedAddress = noTruncate ? id : formatter(id);
        const queryString = new URLSearchParams(queryStrings).toString();
        const queryStringPrefix = queryString ? `?${queryString}` : '';

        async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement>) {
            event.stopPropagation();
            if (!navigator.clipboard) {
                return;
            }
            if (copyText) {
                try {
                    await navigator.clipboard.writeText(copyText);
                    onCopySuccess?.(event, copyText);
                } catch (error) {
                    console.error('Failed to copy:', error);
                    onCopyError?.(error, copyText);
                }
            }
        }

        return (
            <div className="flex flex-row items-center gap-x-xxs">
                <Link
                    className="text-primary-30 dark:text-primary-80"
                    variant="mono"
                    to={`/${base}/${encodeURI(id)}${queryStringPrefix}`}
                    {...props}
                >
                    {label || truncatedAddress}
                </Link>
                {copyText && (
                    <ButtonUnstyled onClick={handleCopyClick}>
                        <Copy className="text-neutral-60 dark:text-neutral-40" />
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
