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
                <span className="text-label-lg text-neutral-40 dark:text-neutral-60">{title}</span>
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
                            <CheckmarkFilled className="h-4 w-4 text-primary-30 dark:text-primary-80" />
                        </Tooltip>
                    ) : null}
                </div>
            </div>
            <div className="flex flex-col gap-xs">
                {accounts.map((anAccount) => (
                    <div className="rounded-xl border border-shader-neutral-light-8">
                        <AccountListItem key={anAccount.id} account={anAccount} icon={<Key />} />
                    </div>
                ))}
            </div>
        </div>
    );
}
