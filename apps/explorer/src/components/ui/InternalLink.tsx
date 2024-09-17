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
            <Link variant="mono" to={`/${base}/${encodeURI(id)}${queryStringPrefix}`} {...props}>
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
