// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useLocation, Link } from 'react-router-dom';
import { Navbar, type NavbarItemWithId, Tooltip, TooltipPosition } from '@iota/apps-ui-kit';
import {
    Activity,
    Apps,
    Assets,
    Home,
    IotaLogoMark,
    Keystone,
    Ledger,
    Passkey,
} from '@iota/apps-ui-icons';
import { useAppSelector, useActiveAccount } from '_hooks';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';
import { useNextMenuUrl } from '_components';
import { WalletSettingsButton } from '_components';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { isKeystoneAccountSerializedUI } from '_src/background/accounts/keystoneAccount';
import { isPasskeyAccountSerializedUI } from '_src/background/accounts/passkeyAccount';
import { useGetDefaultIotaName, getCustomNetwork } from '@iota/core';
import { formatAccountName } from '../../helpers';
import { getNetwork, Network } from '@iota/iota-sdk/client';

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
        { id: 'home', icon: <Home />, path: '/tokens' },
        { id: 'assets', icon: <Assets />, path: '/nfts' },
        { id: 'apps', icon: <Apps />, path: '/apps' },
        { id: 'activity', icon: <Activity />, path: '/transactions' },
    ];

    const activeId = NAVBAR_ITEMS.find((item) => location.pathname.startsWith(item.path))?.id || '';

    function handleItemClick(id: string) {
        const item = NAVBAR_ITEMS.find((item) => item.id === id);
        if (item && !item.isDisabled) {
            navigate(item.path);
        }
    }

    if (useSidebar) {
        return (
            <div className="flex h-full w-16 shrink-0 flex-col items-center border-r border-shader-neutral-light-8 bg-iota-neutral-100 py-xs dark:border-shader-neutral-dark-8 dark:bg-iota-neutral-6">
                {/* Account icon at the top */}
                <Tooltip text={accountName} position={TooltipPosition.Right}>
                    <Link
                        to="/accounts/manage"
                        className="mb-sm flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-iota-primary-30 no-underline [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-white"
                        data-testid="accounts-manage"
                    >
                        {isLedgerAccount ? (
                            <Ledger />
                        ) : isKeystoneAccount ? (
                            <Keystone />
                        ) : isPasskeyAccount ? (
                            <Passkey />
                        ) : (
                            <IotaLogoMark />
                        )}
                    </Link>
                </Tooltip>

                {/* Nav items */}
                {NAVBAR_ITEMS.map((item) => (
                    <div
                        key={item.id}
                        className="flex items-center px-xs py-xxs"
                        data-testid={`nav-${item.id}`}
                    >
                        <Navbar items={[item]} activeId={activeId} onClickItem={handleItemClick} />
                    </div>
                ))}

                {/* Network indicator + settings at the bottom */}
                <div className="mt-auto flex flex-col items-center gap-xs pb-xs">
                    <Tooltip text={networkName} position={TooltipPosition.Right}>
                        <Link
                            to={networkUrl}
                            className="flex h-8 w-8 items-center justify-center rounded-full no-underline hover:bg-iota-neutral-90 dark:hover:bg-iota-neutral-12"
                            aria-label={`Network: ${networkName}`}
                        >
                            <span className="bg-iota-primary h-2.5 w-2.5 rounded-full" />
                        </Link>
                    </Tooltip>
                    <WalletSettingsButton />
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
