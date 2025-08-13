// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Address } from '@iota/apps-ui-kit';
import { useGetDefaultIotaName } from '../../hooks';
import clsx from 'clsx';
import { truncateString } from '../../utils';
import { formatAddress } from '@iota/iota-sdk/utils';

interface NamedAddressProps extends Omit<React.ComponentProps<typeof Address>, 'text'> {
    address: string;
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
    const { data: iotaName } = useGetDefaultIotaName(address);
    const formattedAddress = formatAddress(address);

    return (
        <div
            className={clsx(
                'flex flex-col gap-y-xxs items-center',
                addMarginRightToCenter ? '-mr-xl' : '',
            )}
        >
            {iotaName ? (
                <span className="text-label-md dark:text-iota-neutral-92 text-iota-neutral-10 -ml-xl">
                    {truncateString(iotaName, 12)}
                </span>
            ) : null}
            <Address
                text={formattedAddress}
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
