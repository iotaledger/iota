// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'clsx';
import { Address } from '@iota/apps-ui-kit';
import { useGetIotaName } from '../../hooks';

interface NamedAddressProps {
    address: string;
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function NamedAddress({ address, onCopy }: NamedAddressProps): React.JSX.Element {
    const { data: defaultName } = useGetIotaName(address);

    console.log('defaultName', defaultName);
    return (
        <div className="flex flex-col gap-y-xxs">
            <Address text={address} />
            {defaultName ? (
                <div
                    className={cx(
                        'flex items-center gap-xs text-iota-neutral-40 dark:text-iota-neutral-60',
                    )}
                >
                    {defaultName.name}
                </div>
            ) : null}
        </div>
    );
}
