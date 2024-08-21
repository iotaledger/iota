// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl, Overlay } from '_components';
import { useAppSelector } from '_hooks';
import { getCustomNetwork } from '_src/shared/api-env';
import { FAQ_LINK, ToS_LINK } from '_src/shared/constants';
import { formatAutoLock, useAutoLockMinutes } from '_src/ui/app/hooks/useAutoLockMinutes';
import FaucetRequestButton from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import Browser from 'webextension-polyfill';
import MenuListItem from './MenuListItem';
import { Link, useNavigate } from 'react-router-dom';
import { useQueryClient, useMutation } from '@tanstack/react-query';
import { persister } from '_src/ui/app/helpers/queryClient';
import { useBackgroundClient } from '_src/ui/app/hooks/useBackgroundClient';
import { useState } from 'react';
import { ConfirmationModal } from '_src/ui/app/shared/ConfirmationModal';
import { DarkMode, Globe, Info, LockLocked, LockUnlocked, Logout } from '@iota/ui-icons';
import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';

function MenuList() {
    const navigate = useNavigate();
    const activeAccount = useActiveAccount();
    const networkUrl = useNextMenuUrl(true, '/network');
    const autoLockUrl = useNextMenuUrl(true, '/auto-lock');
    const network = useAppSelector((state) => state.app.network);
    const networkConfig = network === Network.Custom ? getCustomNetwork() : getNetwork(network);
    const version = Browser.runtime.getManifest().version;
    const autoLockInterval = useAutoLockMinutes();

    // Logout
    const [isLogoutDialogOpen, setIsLogoutDialogOpen] = useState(false);
    const backgroundClient = useBackgroundClient();
    const queryClient = useQueryClient();
    const logoutMutation = useMutation({
        mutationKey: ['logout', 'clear wallet'],
        mutationFn: async () => {
            // ampli.client.reset();
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

    function onFAQClick() {
        window.open(FAQ_LINK, '_blank', 'noopener noreferrer');
    }

    const autoLockSubtitle = handleAutoLockSubtitle();
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
            title: 'FAQ',
            icon: <Info />,
            onClick: onFAQClick,
        },
        {
            title: 'Themes',
            icon: <DarkMode />,
            onClick: () => {},
            isDisabled: true,
        },
        {
            title: 'Reset',
            icon: <Logout />,
            onClick: () => setIsLogoutDialogOpen(true),
            isDisabled: true,
            isHidden: true,
        },
    ];

    return (
        <Overlay showModal title="Settings" closeOverlay={() => navigate('/')}>
            <div className="flex h-full flex-col justify-between">
                <div className="flex flex-col">
                    {MENU_ITEMS.map((item, index) => (
                        <MenuListItem key={index} {...item} />
                    ))}
                    <ConfirmationModal
                        isOpen={isLogoutDialogOpen}
                        confirmText="Logout"
                        confirmStyle="outlineWarning"
                        title="Are you sure you want to Logout?"
                        hint="You will need to set up all your accounts again."
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
                        <span className="text-label-sm text-neutral-40">
                            IOTA Wallet v{version}
                        </span>
                        <Link
                            to={ToS_LINK}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-label-sm text-primary-30"
                        >
                            Terms of Service
                        </Link>
                    </div>
                </div>
            </div>
        </Overlay>
    );
}

export default MenuList;
