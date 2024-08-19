// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';
import { useResolveIotaNSName } from '@iota/core';
import { ArrowUpRight12, Copy12 } from '@iota/icons';
import { formatAddress } from '@iota/iota-sdk/utils';
import cn from 'clsx';
import { forwardRef, type ReactNode } from 'react';

import { useAccounts } from '../../hooks/useAccounts';
import { useCopyToClipboard } from '../../hooks/useCopyToClipboard';
import { useExplorerLink } from '../../hooks/useExplorerLink';
import { ExplorerLinkType, IconButton } from '_components';
import { EditableAccountName } from './EditableAccountName';

interface AccountItemProps {
    accountID: string;
    icon?: ReactNode;
    after?: ReactNode;
    footer?: ReactNode;
    disabled?: boolean;
    gradient?: boolean;
    selected?: boolean; // whether the account is selected in the context of a multi-select
    isActiveAccount?: boolean; // whether the account is the active account in the context of the account list
    background?: 'gradient';
    editable?: boolean;
    hideExplorerLink?: boolean;
    hideCopy?: boolean;
}

export const AccountItem = forwardRef<HTMLDivElement, AccountItemProps>(
    (
        {
            background,
            selected,
            isActiveAccount,
            disabled,
            icon,
            accountID,
            after,
            footer,
            editable,
            hideExplorerLink,
            hideCopy,
            ...props
        },
        ref,
    ) => {
        const { data: accounts } = useAccounts();
        const account = accounts?.find((account) => account.id === accountID);
        const { data: domainName } = useResolveIotaNSName(account?.address);
        const accountName =
            account?.nickname ?? domainName ?? formatAddress(account?.address || '');
        const copyAddress = useCopyToClipboard(account?.address || '', {
            copySuccessMessage: 'Address copied',
        });
        const explorerHref = useExplorerLink({
            type: ExplorerLinkType.Address,
            address: account?.address,
        });
        if (!account) return null;

        return (
            <div
                ref={ref}
                className={cn(
                    'border-hero/10 group flex cursor-pointer flex-col gap-3 rounded-xl border border-solid bg-white/40 px-4 py-3',
                    'hover:border-hero/20 hover:bg-white/80',
                    { 'cursor-auto bg-white/80 shadow-card-soft': selected },
                    { 'bg-white/80': isActiveAccount },
                    { '!bg-hero/10 border-none shadow-none hover:bg-white/40': disabled },
                    { 'bg-gradients-graph-cards': background === 'gradient' },
                )}
                {...props}
            >
                <div className="flex items-center justify-start gap-3">
                    <div className="mt-0.5 self-start">{icon}</div>
                    <div className="flex flex-col items-start gap-1 overflow-hidden">
                        {!isActiveAccount && !editable ? (
                            <Text variant="pBody" weight="semibold" color="steel-darker" truncate>
                                {accountName}
                            </Text>
                        ) : (
                            <EditableAccountName accountID={account.id} name={accountName} />
                        )}
                        <div className="text-steel-dark flex gap-1.5 leading-none">
                            <Text variant="subtitle" weight="semibold" truncate>
                                {formatAddress(account.address)}
                            </Text>
                            <div className="flex gap-1 opacity-0 duration-100 group-hover:opacity-100">
                                {hideCopy ? null : (
                                    <IconButton
                                        variant="transparent"
                                        icon={<Copy12 className="h-2.5 w-2.5" />}
                                        onClick={copyAddress}
                                    />
                                )}
                                {hideExplorerLink || !explorerHref ? null : (
                                    <IconButton
                                        variant="transparent"
                                        title="View on Explorer"
                                        href={explorerHref}
                                        icon={<ArrowUpRight12 className="h-2.5 w-2.5" />}
                                        onClick={(e) => e.stopPropagation()}
                                    />
                                )}
                            </div>
                        </div>
                    </div>
                    {after}
                </div>
                {footer}
            </div>
        );
    },
);
