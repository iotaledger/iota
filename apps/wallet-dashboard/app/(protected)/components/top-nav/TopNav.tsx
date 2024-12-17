// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, Button, ButtonType } from '@iota/apps-ui-kit';
import { ThemeSwitcher } from '@iota/core';
import { ConnectButton } from '@iota/dapp-kit';
import { Settings } from '@iota/ui-icons';

export function TopNav() {
    return (
        <div className="flex w-full flex-row items-center justify-end gap-md py-xs--rs">
            <Badge label="Mainnet" type={BadgeType.PrimarySoft} />
            <ConnectButton size="md" />
            <ThemeSwitcher />
            <Button icon={<Settings />} type={ButtonType.Ghost} />
        </div>
    );
}
