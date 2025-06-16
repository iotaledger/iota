// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl, Overlay } from '_components';
import {
    useAppSelector,
    formatAutoLock,
    useAutoLockMinutes,
    useBackgroundClient,
    useActiveAccount,
} from '_hooks';
import { FaucetRequestButton } from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import Browser from 'webextension-polyfill';
import { Link, useNavigate } from 'react-router-dom';
import { useQueryClient, useMutation } from '@tanstack/react-query';
import { persister } from '_src/ui/app/helpers/queryClient';
import { useState } from 'react';
import { ConfirmationModal } from '_src/ui/app/shared/ConfirmationModal';
import {
    DarkMode,
    Globe,
    Info,
    LockLocked,
    LockUnlocked,
    Logout,
    Expand,
    Exclamation,
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
} from '@iota/apps-ui-kit';
import { ampli } from '_src/shared/analytics/ampli';
import { useTheme, getCustomNetwork, FAQ_LINK, ToS_LINK, DISCORD_SUPPORT_LINK } from '@iota/core';

export function MenuList() {
    const { themePreference } = useTheme();
    const navigate = useNavigate();
    const activeAccount = useActiveAccount();
    const networkUrl = useNextMenuUrl(true, '/network');
    const autoLockUrl = useNextMenuUrl(true, '/auto-lock');
    const themeUrl = useNextMenuUrl(true, '/theme');
    const network = useAppSelector((state) => state.app.network);
    const networkConfig = network === Network.Custom ? getCustomNetwork() : getNetwork(network);
    const version = Browser.runtime.getManifest().version;
    const autoLockInterval = useAutoLockMinutes();
    const isAppPopup = useAppSelector((state) => state.app.isAppViewPopup);

    // Logout
    const [isLogoutDialogOpen, setIsLogoutDialogOpen] = useState(false);
    const backgroundClient = useBackgroundClient();
    const queryClient = useQueryClient();
    const logoutMutation = useMutation({
        mutationKey: ['logout', 'clear wallet'],
        mutationFn: async () => {
            ampli.client.reset();
            queryClient.cancelQueries();
            queryClient.clear();
            await persister.removeClient();
            await backgroundClient.clearWallet();
        },
    });

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

    function onSupportClick() {
        window.open(DISCORD_SUPPORT_LINK, '_blank', 'noopener noreferrer');
    }

    function onFAQClick() {
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
            icon: activeAccount?.isLocked ? <LockLocked /> : <LockUnlocked />,
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
            icon: <Exclamation />,
            onClick: onSupportClick,
        },
        {
            title: 'Expand View',
            icon: <Expand />,
            onClick: () =>
                window.open(window.location.href.split('?')[0], '_blank', 'noopener noreferrer'),
            hidden: !isAppPopup,
        },
        {
            title: 'FAQ',
            icon: <Info />,
            onClick: onFAQClick,
        },
        {
            title: 'Reset',
            icon: <Logout />,
            onClick: () => setIsLogoutDialogOpen(true),
        },
    ];

    return (
        <Overlay showModal title="Settings" closeOverlay={() => navigate('/')}>
            <div className="flex h-full w-full flex-col justify-between">
                <div className="flex flex-col">
                    {MENU_ITEMS.filter((item) => !item.hidden).map((item, index) => (
                        <Card key={index} type={CardType.Default} onClick={item.onClick}>
                            <CardImage type={ImageType.BgSolid}>
                                <div className="flex h-10 w-10 items-center justify-center rounded-full  text-neutral-10 dark:text-neutral-92 [&_svg]:h-5 [&_svg]:w-5">
                                    <span className="text-2xl">{item.icon}</span>
                                </div>
                            </CardImage>
                            <CardBody title={item.title} subtitle={item.subtitle} />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                    ))}
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
                        <span className="text-label-sm text-neutral-40 dark:text-neutral-60">
                            IOTA Wallet v{version}
                        </span>
                        <Link
                            to={ToS_LINK}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-label-sm text-primary-30 dark:text-primary-80"
                        >
                            Terms of Service
                        </Link>
                    </div>
                </div>
            </div>
        </Overlay>
    );
}
