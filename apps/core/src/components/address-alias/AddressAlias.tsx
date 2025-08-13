// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy, IotaLogoMark } from '@iota/apps-ui-icons';
import cx from 'clsx';
import { ButtonUnstyled } from '@iota/apps-ui-kit';
import { useAddressAliasLookup } from '../../hooks';
import { trimOrFormatAddress } from '@iota/iota-sdk/utils';

interface AddressAliasProps {
    address: string;
    noTruncate?: boolean;
    truncateUnknown?: boolean;
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    renderAddress?: (addressToDisplay: string) => React.ReactNode;
    renderAlias?: (addressAlias: string) => React.ReactNode;
}

export function AddressAlias({
    address,
    noTruncate = false,
    truncateUnknown = false,
    onCopy,
    renderAddress,
    renderAlias,
}: AddressAliasProps): React.JSX.Element {
    const getAddressAlias = useAddressAliasLookup();

    const alias = getAddressAlias(address);

    const addressToDisplay =
        noTruncate || !truncateUnknown ? address : trimOrFormatAddress(address);

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
                {renderAddress?.(addressToDisplay) ?? addressToDisplay}

                {onCopy && (
                    <ButtonUnstyled onClick={onCopy}>
                        <Copy className="h-full aspect-square hover:text-opacity-80 transition-colors cursor-pointer text-iota-neutral-60 dark:text-iota-neutral-40" />
                    </ButtonUnstyled>
                )}
            </div>
        </>
    );
}
