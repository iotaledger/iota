// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { CheckFill16 } from '@iota/icons';

import { Link } from '../../shared/Link';
import { Text } from '../../shared/text';
import { Tooltip } from '../../shared/tooltip';
import { AccountListItem } from './AccountListItem';

export type RecoverAccountsGroupProps = {
    title: string;
    accounts: SerializedUIAccount[];
    showRecover?: boolean;
    onRecover?: () => void;
    recoverDone?: boolean;
};

export function RecoverAccountsGroup({
    title,
    accounts,
    showRecover,
    onRecover,
    recoverDone,
}: RecoverAccountsGroupProps) {
    return (
        <div className="flex w-full flex-col items-stretch gap-4">
            <div className="flex flex-nowrap items-center gap-1 px-2">
                <Text variant="caption" weight="semibold" color="steel-dark">
                    {title}
                </Text>
                <div className="flex h-px flex-1 flex-shrink-0 bg-gray-45" />
                <div>
                    {showRecover && !recoverDone ? (
                        <Link
                            size="bodySmall"
                            color="hero"
                            weight="semibold"
                            text="Recover"
                            onClick={onRecover}
                        />
                    ) : null}
                    {recoverDone ? (
                        <Tooltip tip="Recovery process done">
                            <CheckFill16 className="h-4 w-4 text-success" />
                        </Tooltip>
                    ) : null}
                </div>
            </div>
            {accounts.map((anAccount) => (
                <AccountListItem key={anAccount.id} account={anAccount} />
            ))}
        </div>
    );
}
