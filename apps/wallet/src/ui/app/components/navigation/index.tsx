// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useLocation, Link } from 'react-router-dom';
import { type NavbarItemWithId } from '@iota/apps-ui-kit';
import { Activity, Apps, Assets, Globe, Home, Settings } from '@iota/apps-ui-icons';
import { useAppSelector } from '_hooks';
import { useMenuIsOpen, useNextMenuUrl } from '_components';
import { getCustomNetwork } from '@iota/core';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import cx from 'clsx';
import { motion } from 'framer-motion';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';

type NavbarItemWithPath = NavbarItemWithId & {
    path: string;
};

const NAVBAR_ITEMS: NavbarItemWithPath[] = [
    { id: 'home', icon: <Home />, text: 'Home', path: '/tokens' },
    { id: 'assets', icon: <Assets />, text: 'Assets', path: '/nfts' },
    { id: 'apps', icon: <Apps />, text: 'Apps', path: '/apps' },
    { id: 'activity', icon: <Activity />, text: 'Activity', path: '/transactions' },
];

export function Navigation() {
    const navigate = useNavigate();
    const location = useLocation();
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    const isFullScreen = extensionViewType === ExtensionViewType.FullScreen;

    const network = useAppSelector(({ app }) => app.network);
    const networkConfig = network === Network.Custom ? getCustomNetwork() : getNetwork(network);
    const networkName = networkConfig?.name ?? network;

    const activeId = NAVBAR_ITEMS.find((item) => location.pathname.startsWith(item.path))?.id || '';

    function handleItemClick(id: string) {
        const item = NAVBAR_ITEMS.find((item) => item.id === id);
        if (item && !item.isDisabled) {
            navigate(item.path);
        }
    }

    const isMenuOpen = useMenuIsOpen();
    const menuUrl = useNextMenuUrl(!isMenuOpen, '/');
    const isNetworkActive = !isMenuOpen && location.pathname.startsWith('/network');

    const sidebarWidth = isFullScreen ? 'w-44' : 'w-16';

    return (
        <div
            className={cx(
                'flex h-full shrink-0 flex-col border-r border-shader-neutral-light-8 bg-iota-neutral-100 px-xs pb-sm pt-lg dark:border-shader-neutral-dark-8 dark:bg-iota-neutral-6',
                sidebarWidth,
            )}
        >
            <motion.div
                className="flex w-full flex-col gap-xs"
                initial="hidden"
                animate="visible"
                variants={{
                    hidden: {},
                    visible: { transition: { staggerChildren: 0.03, delayChildren: 0.04 } },
                }}
            >
                {NAVBAR_ITEMS.map((item) => (
                    <motion.div
                        key={item.id}
                        role="button"
                        tabIndex={0}
                        aria-label={item.text}
                        aria-current={item.id === activeId ? 'page' : undefined}
                        onClick={() => handleItemClick(item.id)}
                        onKeyDown={(e) => {
                            if (e.key === 'Enter' || e.key === ' ') {
                                e.preventDefault();
                                handleItemClick(item.id);
                            }
                        }}
                        data-testid={`nav-${item.id}`}
                        className={cx(
                            'state-layer-secondary relative flex cursor-pointer flex-row items-center rounded-full',
                            isFullScreen ? 'gap-sm px-xs py-xs' : 'justify-center py-xs',
                        )}
                        variants={{
                            hidden: { opacity: 0 },
                            visible: {
                                opacity: 1,
                                transition: { duration: 0.12, ease: 'easeOut' },
                            },
                        }}
                        whileTap={{ scale: 0.94 }}
                        transition={{ duration: 0.08, ease: [0.25, 1, 0.5, 1] }}
                    >
                        {item.id === activeId && (
                            <motion.div
                                layoutId="sidebar-active-indicator"
                                className="absolute inset-0 rounded-full bg-shader-primary-light-12 dark:bg-shader-primary-dark-12"
                                transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
                            />
                        )}
                        <div
                            className={cx(
                                'relative inline-flex transition-colors duration-[220ms] ease-[cubic-bezier(0.16,1,0.3,1)] [&_svg]:h-6 [&_svg]:w-6',
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
                                    'relative text-label-lg transition-colors duration-[220ms] ease-[cubic-bezier(0.16,1,0.3,1)]',
                                    item.id === activeId
                                        ? 'navbar-item-label-selected-color'
                                        : 'navbar-item-label-color',
                                )}
                            >
                                {item.text}
                            </span>
                        )}
                    </motion.div>
                ))}
            </motion.div>

            <div className="mt-auto flex w-full flex-col gap-y-sm">
                <Link
                    to="/network"
                    className={cx(
                        'flex flex-row items-center rounded-full no-underline hover:bg-shader-neutral-light-8 dark:hover:bg-shader-neutral-dark-8',
                        isFullScreen ? 'gap-sm px-xs py-xs' : 'justify-center py-xs',
                        isNetworkActive &&
                            'bg-shader-primary-light-12 dark:bg-shader-primary-dark-12',
                    )}
                    aria-label={`Network: ${networkName}`}
                    aria-current={isNetworkActive ? 'page' : undefined}
                >
                    <div
                        className={cx(
                            'flex shrink-0 items-center justify-center [&_svg]:h-6 [&_svg]:w-6',
                            isNetworkActive
                                ? 'navbar-item-icon-selected-color'
                                : 'navbar-item-icon-color',
                        )}
                    >
                        <Globe />
                    </div>
                    {isFullScreen && (
                        <span
                            className={cx(
                                'truncate text-label-lg',
                                isNetworkActive
                                    ? 'navbar-item-label-selected-color'
                                    : 'navbar-item-label-color',
                            )}
                        >
                            {networkName}
                        </span>
                    )}
                </Link>

                <Link
                    to={menuUrl}
                    className={cx(
                        'flex flex-row items-center rounded-full no-underline hover:bg-shader-neutral-light-8 dark:hover:bg-shader-neutral-dark-8',
                        isFullScreen ? 'gap-sm px-xs py-xs' : 'justify-center py-xs',
                        isMenuOpen && 'bg-shader-primary-light-12 dark:bg-shader-primary-dark-12',
                    )}
                    aria-label={isMenuOpen ? 'Close settings menu' : 'Open settings menu'}
                    aria-current={isMenuOpen ? 'page' : undefined}
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
                        <Settings />
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
