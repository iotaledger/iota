// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SettingsDialog, useSettingsDialog } from '@/components';
import { Badge, BadgeType, Button, ButtonType } from '@iota/apps-ui-kit';
import { ConnectButton, useIotaClientContext } from '@iota/dapp-kit';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import { ThemeSwitcher } from '@iota/core';
import { Settings } from '@iota/ui-icons';

export function TopNav() {
    const { network } = useIotaClientContext();
    const { name: networkName } = getNetwork(network);
    const {
        isSettingsDialogOpen,
        settingsDialogView,
        setSettingsDialogView,
        onCloseSettingsDialogClick,
        onOpenSettingsDialogClick,
    } = useSettingsDialog();

    return (
        <div className="flex w-full flex-row items-center justify-end gap-md py-xs--rs">
            <Badge
                label={networkName}
                type={network === Network.Mainnet ? BadgeType.PrimarySoft : BadgeType.Neutral}
            />
            <ConnectButton size="md" />
            <SettingsDialog
                isOpen={isSettingsDialogOpen}
                handleClose={onCloseSettingsDialogClick}
                view={settingsDialogView}
                setView={setSettingsDialogView}
            />
            <ThemeSwitcher />
            <Button
                icon={<Settings />}
                type={ButtonType.Ghost}
                onClick={onOpenSettingsDialogClick}
            />
        </div>
    );
}
