// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, Button, ButtonType } from '@iota/apps-ui-kit';
import { ConnectButton } from '@iota/dapp-kit';
import { Settings } from '@iota/ui-icons';

export function TopNav() {
    return (
        <div className="flex h-[52px] w-full flex-row items-center justify-end gap-md py-xs--rs">
            <Badge label="Mainnet" type={BadgeType.PrimarySoft} />
            <ConnectButton />
            <Button icon={<Settings />} type={ButtonType.Ghost} />
        </div>
    );
}
