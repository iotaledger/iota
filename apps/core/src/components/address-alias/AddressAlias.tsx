// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy, IotaLogoMark } from '@iota/apps-ui-icons';
import { useAddressAliasLookup, type GetAddressAliasParams } from '../../hooks';
import cx from 'clsx';
import { ButtonUnstyled } from '@iota/apps-ui-kit';

interface AddressAliasProps extends GetAddressAliasParams {
    noFormatAddress?: boolean;
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    renderAddress?: (formattedAddress: string) => React.ReactNode;
    renderAlias?: (addressAlias: string) => React.ReactNode;
}

export function AddressAlias({
    address,
    formatUnknownAddress = true,
    noFormatAddress,
    onCopy,
    renderAddress,
    renderAlias,
}: AddressAliasProps): React.JSX.Element {
    const getAddressAlias = useAddressAliasLookup();

    const { address: formattedAddress, alias } = getAddressAlias({
        address,
        formatUnknownAddress,
    });

    const displayAddress = noFormatAddress ? address : formattedAddress;
    return (
        <>
            {alias && (
                <div
                    className={cx(
                        'flex items-center gap-xs text-iota-neutral-40 dark:text-iota-neutral-60',
                    )}
                >
                    <IotaLogoMark className="h-full aspect-square shrink-0" />
                    {renderAlias?.(alias) ?? alias}
                </div>
            )}

            <div className="flex flex-row items-center gap-xxs">
                {renderAddress?.(displayAddress) ?? displayAddress}

                {onCopy && (
                    <ButtonUnstyled onClick={onCopy}>
                        <Copy className="h-full aspect-square hover:text-opacity-80 transition-colors cursor-pointer text-iota-neutral-60 dark:text-iota-neutral-40" />
                    </ButtonUnstyled>
                )}
            </div>
        </>
    );
}
