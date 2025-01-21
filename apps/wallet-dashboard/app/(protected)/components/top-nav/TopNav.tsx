// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SettingsDialog, useSettingsDialog } from '@/components';
import { Badge, BadgeType, Button, ButtonType } from '@iota/apps-ui-kit';
import { ConnectButton } from '@iota/dapp-kit';
import { Network } from '@iota/iota-sdk/client';
import { toTitleCase, ThemeSwitcher } from '@iota/core';
import { Settings } from '@iota/apps-ui-icons';
import { usePersistedNetwork } from '@/hooks';

export function TopNav() {
    const { persistedNetwork } = usePersistedNetwork();

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
                label={toTitleCase(persistedNetwork)}
                type={
                    persistedNetwork === Network.Mainnet ? BadgeType.PrimarySoft : BadgeType.Neutral
                }
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
