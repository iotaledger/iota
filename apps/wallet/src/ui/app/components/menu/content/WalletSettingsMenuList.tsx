// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl, Overlay, VerifyPasswordModal } from '_components';
import { useAppSelector, formatAutoLock, useAutoLockMinutes, useLogoutMutation } from '_hooks';
import { FaucetRequestButton } from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import Browser from 'webextension-polyfill';
import { Link, useNavigate } from 'react-router-dom';
import { useState } from 'react';
import { ConfirmationModal } from '_src/ui/app/shared/ConfirmationModal';
import {
    DarkMode,
    Globe,
    Info,
    LockLocked,
    Logout,
    Expand,
    Discord,
    SidePanel as SidePanelIcon,
} from '@iota/apps-ui-icons';
import {
    ButtonType,
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageType,
    Toggle,
} from '@iota/apps-ui-kit';
import { ampli } from '_src/shared/analytics/ampli';
import { useTheme, getCustomNetwork, FAQ_LINK, ToS_LINK, DISCORD_SUPPORT_LINK } from '@iota/core';
import { useSidePanel } from '_src/ui/app/hooks/useSidePanel';
import { useSidePanelMutation } from '_src/ui/app/hooks/useSidePanelMutation';
import { SidePanel } from '_src/polyfills/sidepanel';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';
import { openInNewTab } from '_src/shared/utils';

export function MenuList() {
    const { themePreference } = useTheme();
    const navigate = useNavigate();
    const networkUrl = useNextMenuUrl(true, '/network');
    const autoLockUrl = useNextMenuUrl(true, '/auto-lock');
    const themeUrl = useNextMenuUrl(true, '/theme');
    const network = useAppSelector((state) => state.app.network);
    const networkConfig = network === Network.Custom ? getCustomNetwork() : getNetwork(network);
    const version = Browser.runtime.getManifest().version;
    const autoLockInterval = useAutoLockMinutes();
    const sidePanel = useSidePanel();
    const sidePanelMutation = useSidePanelMutation();
    const extensionType = useAppSelector((state) => state.app.extensionViewType);

    // Logout
    const [isLogoutDialogOpen, setIsLogoutDialogOpen] = useState(false);
    const [isPasswordModalVisible, setIsPasswordModalVisible] = useState(false);
    const logoutMutation = useLogoutMutation();

    function handleAutoLockSubtitle(): string {
        if (autoLockInterval.data === null) {
            return 'Not set up';
        }
        if (typeof autoLockInterval.data === 'number') {
            return formatAutoLock(autoLockInterval.data);
        }
        return '';
    }

    function onNetworkClick() {
        navigate(networkUrl);
    }

    function onAutoLockClick() {
        navigate(autoLockUrl);
    }

    function onThemeClick() {
        navigate(themeUrl);
    }
    async function onSidePanelClick(
        _isToggled: boolean,
        event: React.ChangeEvent<HTMLInputElement>,
    ) {
        const isSidePanelVisible = event.target.checked;

        if (!isSidePanelVisible) {
            // Track before the mutation: SidePanel.close() destroys this window, so we must flush before it runs
            ampli.changedSidePanel({ enabled: false });
            await ampli.flush();
        }

        try {
            await sidePanelMutation.mutateAsync(isSidePanelVisible);
        } catch {
            // If the mutation fails, don't track the enabled event
            return;
        }

        if (isSidePanelVisible) {
            // Track after the mutation: the popup is still alive, so it's safe to flush before closing
            ampli.changedSidePanel({ enabled: true });
            await ampli.flush();
            window.close();
        }
    }

    function onSupportClick() {
        ampli.openedLink({
            type: 'discord support',
        });
        window.open(DISCORD_SUPPORT_LINK, '_blank', 'noopener noreferrer');
    }

    function onFAQClick() {
        ampli.openedLink({ type: 'faqs documentation' });
        window.open(FAQ_LINK, '_blank', 'noopener noreferrer');
    }

    const autoLockSubtitle = handleAutoLockSubtitle();
    const themeSubtitle = themePreference.charAt(0).toUpperCase() + themePreference.slice(1);
    const MENU_ITEMS = [
        {
            title: 'Network',
            subtitle: networkConfig.name,
            icon: <Globe />,
            onClick: onNetworkClick,
        },
        {
            title: 'Auto Lock Profile',
            subtitle: autoLockSubtitle,
            icon: <LockLocked />,
            onClick: onAutoLockClick,
        },
        {
            title: 'Themes',
            icon: <DarkMode />,
            subtitle: themeSubtitle,
            onClick: onThemeClick,
        },
        {
            title: 'Get Support',
            icon: <Discord />,
            onClick: onSupportClick,
        },
        {
            title: 'Expand View',
            icon: <Expand />,
            onClick: () => openInNewTab('/tokens'),
            hidden: extensionType !== ExtensionViewType.Popup,
        },
        ...(SidePanel.isSupported()
            ? [
                  {
                      title: 'Side Panel',
                      subtitle: sidePanel.data ? `Enabled` : 'Disabled',
                      icon: <SidePanelIcon />,
                      tailIcon: <Toggle isToggled={!!sidePanel.data} onChange={onSidePanelClick} />,
                  },
              ]
            : []),
        {
            title: 'FAQ',
            icon: <Info />,
            onClick: onFAQClick,
        },
        {
            title: 'Reset',
            icon: <Logout />,
            onClick: () => setIsPasswordModalVisible(true),
        },
    ];

    return (
        <Overlay showModal title="Settings" closeOverlay={() => navigate('/tokens')}>
            <div className="flex h-full w-full flex-col justify-between">
                <div className="flex flex-col">
                    {MENU_ITEMS.filter((item) => !item.hidden).map((item, index) => (
                        <Card key={index} type={CardType.Default} onClick={item.onClick}>
                            <CardImage type={ImageType.BgSolid}>
                                <div className="flex h-10 w-10 items-center justify-center rounded-full  text-iota-neutral-10 dark:text-iota-neutral-92 [&_svg]:h-5 [&_svg]:w-5">
                                    <span className="text-2xl">{item.icon}</span>
                                </div>
                            </CardImage>
                            <CardBody title={item.title} subtitle={item.subtitle} />
                            {item.tailIcon ?? <CardAction type={CardActionType.Link} />}
                        </Card>
                    ))}
                    <VerifyPasswordModal
                        open={isPasswordModalVisible}
                        onVerify={() => {
                            setIsPasswordModalVisible(false);
                            setIsLogoutDialogOpen(true);
                        }}
                        onClose={() => setIsPasswordModalVisible(false)}
                    />
                    <ConfirmationModal
                        isOpen={isLogoutDialogOpen}
                        confirmText="Reset"
                        confirmStyle={ButtonType.Destructive}
                        title="Are you sure you want to reset?"
                        hint="This will clear all your data and you will need to set up all your accounts again."
                        onResponse={async (confirmed) => {
                            setIsLogoutDialogOpen(false);
                            if (confirmed) {
                                await logoutMutation.mutateAsync(undefined, {
                                    onSuccess: () => {
                                        window.location.reload();
                                    },
                                });
                            }
                        }}
                    />
                </div>
                <div className="flex flex-col gap-y-lg">
                    <FaucetRequestButton />
                    <div className="flex flex-row items-center justify-center gap-x-md">
                        <span className="text-label-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                            IOTA Wallet v{version}
                        </span>
                        <Link
                            to={ToS_LINK}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-label-sm text-iota-primary-30 dark:text-iota-primary-80"
                        >
                            Terms of Service
                        </Link>
                    </div>
                </div>
            </div>
        </Overlay>
    );
}
