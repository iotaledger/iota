// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy, IotaLogoMark } from '@iota/apps-ui-icons';
import { useGetAddressAlias } from '../../hooks';
import cx from 'clsx';
import { ButtonUnstyled } from '@iota/apps-ui-kit';

interface AddressAliasProps {
    address: string;
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    noTruncate?: boolean;
    truncateUnknown?: boolean;
    renderAddress?: (formattedAddress: string) => React.ReactNode;
    renderAlias?: (addressAlias: string) => React.ReactNode;
}

export function AddressAlias({
    address,
    noTruncate,
    truncateUnknown = true,
    onCopy,
    renderAddress,
    renderAlias,
}: AddressAliasProps): React.JSX.Element {
    const getAddressAlias = useGetAddressAlias();

    const { address: formattedAddress, alias } = getAddressAlias({
        address,
        truncateUnknown,
    });

    const displayAddress = noTruncate ? address : formattedAddress;
    return (
        <>
            {alias && (
                <div
                    className={cx('flex items-center gap-xs text-neutral-40 dark:text-neutral-60')}
                >
                    <IotaLogoMark className="h-full aspect-square" />
                    {renderAlias?.(alias) ?? alias}
                </div>
            )}

            <div className="flex flex-row items-center gap-xxs">
                {renderAddress?.(displayAddress) ?? displayAddress}

                {onCopy && (
                    <ButtonUnstyled onClick={onCopy}>
                        <Copy className="h-full aspect-square hover:text-opacity-80 transition-colors cursor-pointer text-neutral-60 dark:text-neutral-40" />
                    </ButtonUnstyled>
                )}
            </div>
        </>
    );
}
