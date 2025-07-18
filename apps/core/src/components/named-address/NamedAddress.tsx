// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Address } from '@iota/apps-ui-kit';
import { useGetIotaName } from '../../hooks';
import clsx from 'clsx';

interface NamedAddressProps {
    address: string;
    isCopyable?: boolean;
    isExternal?: boolean;
    externalLink?: string;
    copyText?: string;
    onCopySuccess?: (e: React.MouseEvent<HTMLButtonElement>, text: string) => void;
    onCopyError?: (e: unknown, text: string) => void;
    onOpen?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    addMarginRightToCenter?: boolean;
}

export function NamedAddress({
    address,
    isCopyable,
    isExternal,
    externalLink,
    copyText,
    onCopySuccess,
    onCopyError,
    onOpen,
    addMarginRightToCenter = false,
}: NamedAddressProps): React.JSX.Element {
    const { data: defaultName } = useGetIotaName(address);

    return (
        <div
            className={clsx(
                'flex flex-col gap-y-xxs',
                defaultName ? ' items-start' : 'items-center',
                addMarginRightToCenter && !defaultName ? '-mr-lg' : '',
            )}
        >
            {defaultName ? (
                <span className="text-label-md bg-names-gradient-primary bg-clip-text text-transparent">
                    {defaultName}
                </span>
            ) : null}
            <Address
                text={address}
                isCopyable={isCopyable}
                isExternal={isExternal}
                externalLink={externalLink}
                copyText={copyText}
                onCopySuccess={onCopySuccess}
                onCopyError={onCopyError}
                onOpen={onOpen}
            />
        </div>
    );
}
