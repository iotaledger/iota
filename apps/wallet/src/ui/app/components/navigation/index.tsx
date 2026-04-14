// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useLocation, Link } from 'react-router-dom';
import { Navbar, type NavbarItemWithId } from '@iota/apps-ui-kit';
import {
    Activity,
    Apps,
    Assets,
    Close,
    Globe,
    Home,
    IotaLogoMark,
    Keystone,
    Ledger,
    Passkey,
    Settings,
} from '@iota/apps-ui-icons';
import { useAppSelector, useActiveAccount } from '_hooks';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';
import { useMenuIsOpen, useNextMenuUrl } from '_components';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { isKeystoneAccountSerializedUI } from '_src/background/accounts/keystoneAccount';
import { isPasskeyAccountSerializedUI } from '_src/background/accounts/passkeyAccount';
import { useGetDefaultIotaName, getCustomNetwork } from '@iota/core';
import { formatAccountName } from '../../helpers';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import cx from 'clsx';

type NavbarItemWithPath = NavbarItemWithId & {
    path: string;
};

export function Navigation() {
    const navigate = useNavigate();
    const location = useLocation();
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    const isFullScreen = extensionViewType === ExtensionViewType.FullScreen;
    const isPopup = extensionViewType === ExtensionViewType.Popup;
    const useSidebar = isFullScreen || isPopup;

    const activeAccount = useActiveAccount();
    const { data: iotaName } = useGetDefaultIotaName(activeAccount?.address);
    const accountName = formatAccountName(
        activeAccount?.nickname,
        iotaName,
        activeAccount?.address,
    );
    const isLedgerAccount = activeAccount && isLedgerAccountSerializedUI(activeAccount);
    const isKeystoneAccount = activeAccount && isKeystoneAccountSerializedUI(activeAccount);
    const isPasskeyAccount = activeAccount && isPasskeyAccountSerializedUI(activeAccount);

    const network = useAppSelector(({ app }) => app.network);
    const networkConfig = network === Network.Custom ? getCustomNetwork() : getNetwork(network);
    const networkName = networkConfig?.name ?? network;

    const networkUrl = useNextMenuUrl(true, '/network');

    const NAVBAR_ITEMS: NavbarItemWithPath[] = [
        { id: 'home', icon: <Home />, text: 'Home', path: '/tokens' },
        { id: 'assets', icon: <Assets />, text: 'Assets', path: '/nfts' },
        { id: 'apps', icon: <Apps />, text: 'Apps', path: '/apps' },
        { id: 'activity', icon: <Activity />, text: 'Activity', path: '/transactions' },
    ];

    const activeId = NAVBAR_ITEMS.find((item) => location.pathname.startsWith(item.path))?.id || '';

    function handleItemClick(id: string) {
        const item = NAVBAR_ITEMS.find((item) => item.id === id);
        if (item && !item.isDisabled) {
            navigate(item.path);
        }
    }

    const isMenuOpen = useMenuIsOpen();
    const menuUrl = useNextMenuUrl(!isMenuOpen, '/');
    const SettingsIcon = isMenuOpen ? Close : Settings;

    if (useSidebar) {
        const sidebarWidth = isFullScreen ? 'w-44' : 'w-16';

        return (
            <div
                className={cx(
                    'flex h-full shrink-0 flex-col border-r border-shader-neutral-light-8 bg-iota-neutral-100 px-xs pb-sm pt-lg dark:border-shader-neutral-dark-8 dark:bg-iota-neutral-6',
                    sidebarWidth,
                )}
            >
                <Link
                    to="/accounts/manage"
                    className={cx(
                        'mb-sm flex flex-row items-center rounded-full no-underline hover:bg-shader-neutral-light-8 dark:hover:bg-shader-neutral-dark-8',
                        isFullScreen ? 'gap-3 px-xs' : 'justify-center px-xs',
                    )}
                    data-testid="accounts-manage"
                >
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-iota-primary-30 [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-white">
                        {isLedgerAccount ? (
                            <Ledger />
                        ) : isKeystoneAccount ? (
                            <Keystone />
                        ) : isPasskeyAccount ? (
                            <Passkey />
                        ) : (
                            <IotaLogoMark />
                        )}
                    </div>
                    {isFullScreen && (
                        <span className="navbar-item-label-color truncate text-label-lg">
                            {accountName}
                        </span>
                    )}
                </Link>

                <div className="flex w-full flex-col gap-xs">
                    {NAVBAR_ITEMS.map((item) => (
                        <div
                            key={item.id}
                            role="button"
                            onClick={() => handleItemClick(item.id)}
                            data-testid={`nav-${item.id}`}
                            className={cx(
                                'state-layer-secondary relative flex cursor-pointer flex-row items-center rounded-full',
                                isFullScreen ? 'gap-3 px-xs py-[6px]' : 'justify-center py-xs',
                            )}
                        >
                            <div
                                className={cx(
                                    'inline-flex [&_svg]:h-6 [&_svg]:w-6',
                                    item.id === activeId
                                        ? 'navbar-item-icon-selected-color'
                                        : 'navbar-item-icon-color',
                                )}
                            >
                                {item.icon}
                            </div>
                            {isFullScreen && (
                                <span
                                    className={cx(
                                        'text-label-lg',
                                        item.id === activeId
                                            ? 'navbar-item-label-selected-color'
                                            : 'navbar-item-label-color',
                                    )}
                                >
                                    {item.text}
                                </span>
                            )}
                        </div>
                    ))}
                </div>

                <div className="mt-auto flex w-full flex-col">
                    <Link
                        to={networkUrl}
                        className={cx(
                            'flex flex-row items-center rounded-full no-underline hover:bg-shader-neutral-light-8 dark:hover:bg-shader-neutral-dark-8',
                            isFullScreen ? 'gap-3 px-xs py-xs' : 'justify-center py-xs',
                        )}
                        aria-label={`Network: ${networkName}`}
                    >
                        <div
                            className={cx(
                                'flex shrink-0 items-center justify-center [&_svg]:h-6 [&_svg]:w-6',
                            )}
                        >
                            {isFullScreen ? (
                                <Globe className="navbar-item-icon-color" />
                            ) : (
                                <Globe className="navbar-item-icon-color" />
                            )}
                        </div>
                        {isFullScreen && (
                            <span className="navbar-item-label-color truncate text-label-lg">
                                {networkName}
                            </span>
                        )}
                    </Link>

                    <Link
                        to={menuUrl}
                        className={cx(
                            'flex flex-row items-center rounded-full no-underline hover:bg-shader-neutral-light-8 dark:hover:bg-shader-neutral-dark-8',
                            isFullScreen ? 'gap-3 px-xs py-xs' : 'justify-center py-xs',
                            isMenuOpen &&
                                'bg-shader-primary-light-12 dark:bg-shader-primary-dark-12',
                        )}
                        aria-label={isMenuOpen ? 'Close settings menu' : 'Open settings menu'}
                        data-testid="wallet-settings-button"
                    >
                        <div
                            className={cx(
                                'flex shrink-0 items-center justify-center [&_svg]:h-6 [&_svg]:w-6',
                                isMenuOpen
                                    ? 'navbar-item-icon-selected-color'
                                    : 'navbar-item-icon-color',
                            )}
                        >
                            <SettingsIcon />
                        </div>
                        {isFullScreen && (
                            <span
                                className={cx(
                                    'truncate text-label-lg',
                                    isMenuOpen
                                        ? 'navbar-item-label-selected-color'
                                        : 'navbar-item-label-color',
                                )}
                            >
                                Settings
                            </span>
                        )}
                    </Link>
                </div>
            </div>
        );
    }

    return (
        <div className="sticky bottom-0 w-full shrink-0 border-b-0 bg-iota-neutral-100 dark:bg-iota-neutral-6">
            <Navbar items={NAVBAR_ITEMS} activeId={activeId} onClickItem={handleItemClick} />
        </div>
    );
}
