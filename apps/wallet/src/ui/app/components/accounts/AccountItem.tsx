// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress } from '@iota/iota-sdk/utils';
import { type ReactNode } from 'react';
import { useExplorerLink, useAccounts, useCopyToClipboard } from '_hooks';
import { ExplorerLinkType } from '_components';
import { Account } from '@iota/apps-ui-kit';
import { formatAccountName } from '../../helpers';
import { useGetDefaultIotaName } from '@iota/core';

interface AccountItemProps {
    accountID: string;
    icon?: ReactNode;
    hideExplorerLink?: boolean;
    hideCopy?: boolean;
}

export function AccountItem({ icon, accountID, hideExplorerLink, hideCopy }: AccountItemProps) {
    const { data: accounts } = useAccounts();
    const account = accounts?.find((account) => account.id === accountID);
    const { data: iotaName } = useGetDefaultIotaName(account?.address);

    const accountName = formatAccountName(account?.nickname, iotaName, account?.address);
    const copyAddress = useCopyToClipboard(account?.address || '', {
        copySuccessMessage: 'Address copied',
        textType: 'address',
    });
    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Address,
        address: account?.address,
    });
    if (!account) return null;

    function handleOpen() {
        const newWindow = window.open(explorerHref!, '_blank', 'noopener,noreferrer');
        if (newWindow) newWindow.opener = null;
    }
    return (
        <div data-amp-mask>
            <Account
                title={accountName}
                subtitle={formatAddress(account.address)}
                onOpen={handleOpen}
                avatarContent={() => <AccountAvatar icon={icon} />}
                onCopy={copyAddress}
                isCopyable={!hideCopy}
                isExternal={!hideExplorerLink}
            />
        </div>
    );
}

function AccountAvatar({ icon }: { icon?: ReactNode }) {
    return (
        <div className="flex h-10 w-10 items-center justify-center rounded-full [&_svg]:h-5 [&_svg]:w-5 ">
            {icon}
        </div>
    );
}
