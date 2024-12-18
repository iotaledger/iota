// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Wallet } from '_src/shared/qredo-api';
import { BadgeLabel } from '_src/ui/app/components/BadgeLabel';
import { Text } from '_src/ui/app/shared/text';
import { CheckFill16 } from '@iota/icons';
import { formatAddress } from '@iota/iota-sdk/utils';
import cn from 'clsx';

export type QredoAccountItemProps = Wallet & {
    selected: boolean;
    onClick: () => void;
};

export function QredoAccountItem({ selected, address, onClick, labels }: QredoAccountItemProps) {
    return (
        <div
            className="group flex cursor-pointer flex-nowrap items-center gap-3 py-4"
            onClick={onClick}
        >
            <CheckFill16
                className={cn('h-4 w-4 flex-shrink-0 text-gray-45', {
                    'text-success': selected,
                    'group-hover:text-gray-60': !selected,
                })}
            />
            <div className="flex flex-col flex-nowrap gap-2">
                <Text color={selected ? 'gray-90' : 'steel-darker'}>{formatAddress(address)}</Text>
                {labels.length ? (
                    <div className="flex flex-wrap gap-1">
                        {labels.map(({ key, value }) => (
                            <BadgeLabel key={key} label={value} />
                        ))}
                    </div>
                ) : null}
            </div>
        </div>
    );
}
