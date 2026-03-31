// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SettingsDialog, useSettingsDialog } from '@/components';
import { Badge, BadgeType, Button, ButtonType } from '@iota/apps-ui-kit';
import { ConnectButton } from '@iota/dapp-kit';
import { Network } from '@iota/iota-sdk/client';
import { toTitleCase, ThemeSwitcher, Feature, useFeatureEnabledByNetwork } from '@iota/core';
import { Settings } from '@iota/apps-ui-icons';
import { usePersistedNetwork } from '@/hooks';
import { ampli } from '@/lib/utils/analytics';

export function TopNav() {
    const { persistedNetwork } = usePersistedNetwork();
    const iotaNamesEnabled = useFeatureEnabledByNetwork(
        Feature.IotaNames,
        persistedNetwork as Network,
    );

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
            <div data-amp-mask>
                <ConnectButton
                    size="md"
                    iotaNamesEnabled={iotaNamesEnabled}
                    onConnected={(args) => {
                        ampli.connectedWallet({ wallet: args.wallet.name });
                    }}
                />
            </div>
            <SettingsDialog
                isOpen={isSettingsDialogOpen}
                handleClose={onCloseSettingsDialogClick}
                view={settingsDialogView}
                setView={setSettingsDialogView}
            />
            <ThemeSwitcher onThemeChange={(theme) => ampli.changedTheme({ theme })} />
            <Button
                icon={<Settings />}
                type={ButtonType.Ghost}
                onClick={onOpenSettingsDialogClick}
                aria-label="Settings"
            />
        </div>
    );
}
