// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { AccountListItem } from './AccountListItem';
import { Button, ButtonSize, ButtonType, Tooltip, TooltipPosition } from '@iota/apps-ui-kit';
import { CheckmarkFilled, Key } from '@iota/apps-ui-icons';

export interface RecoverAccountsGroupProps {
    title: string;
    accounts: SerializedUIAccount[];
    showRecover?: boolean;
    onRecover?: () => void;
    recoverDone?: boolean;
}

export function RecoverAccountsGroup({
    title,
    accounts,
    showRecover,
    onRecover,
    recoverDone,
}: RecoverAccountsGroupProps) {
    return (
        <div className="flex w-full flex-col items-stretch gap-xs">
            <div className="flex h-10 w-full flex-nowrap items-center justify-between">
                <span className="text-label-lg text-iota-neutral-40 dark:text-iota-neutral-60">
                    {title}
                </span>
                <div className="flex items-center overflow-visible">
                    {showRecover && !recoverDone ? (
                        <Button
                            size={ButtonSize.Small}
                            type={ButtonType.Secondary}
                            text="Recover"
                            onClick={onRecover}
                        />
                    ) : null}
                    {recoverDone ? (
                        <Tooltip text="Recovery process done" position={TooltipPosition.Left}>
                            <CheckmarkFilled className="h-4 w-4 text-iota-primary-30 dark:text-iota-primary-80" />
                        </Tooltip>
                    ) : null}
                </div>
            </div>
            <div className="flex flex-col gap-xs">
                {accounts.map((anAccount) => (
                    <div
                        key={anAccount.id}
                        className="border-shader-iota-neutral-light-8 rounded-xl border"
                    >
                        <AccountListItem key={anAccount.id} account={anAccount} icon={<Key />} />
                    </div>
                ))}
            </div>
        </div>
    );
}
