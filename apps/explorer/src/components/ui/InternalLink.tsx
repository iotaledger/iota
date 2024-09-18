// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress, formatDigest } from '@iota/iota-sdk/utils';
import { type ReactNode } from 'react';

import { Link, type LinkProps } from '~/components/ui';

interface BaseInternalLinkProps extends LinkProps {
    noTruncate?: boolean;
    label?: string | ReactNode;
    queryStrings?: Record<string, string>;
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
        ...props
    }: BaseInternalLinkProps & Record<T, string>) => {
        const truncatedAddress = noTruncate ? id : formatter(id);
        const queryString = new URLSearchParams(queryStrings).toString();
        const queryStringPrefix = queryString ? `?${queryString}` : '';

        return (
            <Link
                className="text-primary-30 dark:text-primary-80"
                variant="mono"
                to={`/${base}/${encodeURI(id)}${queryStringPrefix}`}
                {...props}
            >
                {label || truncatedAddress}
            </Link>
        );
    };
}

export const EpochLink = createInternalLink('epoch', 'epoch');
export const CheckpointLink = createInternalLink('checkpoint', 'digest', formatAddress);
export const CheckpointSequenceLink = createInternalLink('checkpoint', 'sequence');
export const AddressLink = createInternalLink('address', 'address', (addressOrNs) =>
    formatAddress(addressOrNs),
);
export const ObjectLink = createInternalLink('object', 'objectId', formatAddress);
export const TransactionLink = createInternalLink('txblock', 'digest', formatDigest);
export const ValidatorLink = createInternalLink('validator', 'address', formatAddress);

// This will ultimately replace createInternalLink.
export function createLinkTo<T extends string>(
    base: string,
    propName: T,
): (args: { queryStrings?: Record<string, string> } & Record<T, string>) => string {
    return ({ [propName]: id, queryStrings = {} }) => {
        const queryString = new URLSearchParams(queryStrings).toString();
        const queryStringPrefix = queryString ? `?${queryString}` : '';

        return `/${base}/${encodeURI(id)}${queryStringPrefix}`;
    };
}

export const transactionToLink = createLinkTo('txblock', 'digest');
export const checkpointToLink = createLinkTo('checkpoint', 'digest');
export const epochToLink = createLinkTo('epoch', 'epoch');
export const addressToLink = createLinkTo('address', 'address');
export const checkpointSequenceToLink = createLinkTo('checkpoint', 'sequence');
export const objectToLink = createLinkTo('object', 'objectId');
