// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Search } from '@iota/apps-ui-icons';
import { Input, InputType } from '@iota/apps-ui-kit';
import { useCurrentWallet } from '@iota/dapp-kit';
import Link from 'next/link';
import { usePathname } from 'next/navigation';

import { NamesLogoWeb } from '@/components/svgs';
import { MY_NAMES_ROUTE, PROTECTED_ROUTES, PUBLIC_ROUTES } from '@/lib/constants';
import { useAvailabilityCheckDialog } from '@/stores/useAvailabilityCheckDialog';

import { ConnectButton } from '../../buttons/ConnectButton';
import { NavbarMobile } from './NavbarMobile';

export function Navbar({ isBannerVisible }: { isBannerVisible: boolean }) {
    const { isConnected } = useCurrentWallet();
    const pathname = usePathname();
    const { open } = useAvailabilityCheckDialog();

    const isAllowedSearchOnPage = [MY_NAMES_ROUTE.path].includes(pathname);

    function toggleSearchDialog() {
        open({
            autoFocusInput: true,
        });
    }

    const ROUTES = isConnected ? [...PROTECTED_ROUTES, ...PUBLIC_ROUTES] : PUBLIC_ROUTES;

    return (
        <nav
            id="top-navbar"
            data-testid="top-navbar"
            className="w-full z-50 backdrop-blur-lg transition-all duration-300"
        >
            <div className="px-lg py-md flex flex-col gap-y-sm">
                <div className="flex flex-row justify-between items-center gap-x-md">
                    <div className="flex flex-row gap-x-lg items-center">
                        <Link href="/" aria-label="Go to homepage">
                            <NamesLogoWeb className="w-32 h-2xl text-names-primary-100" />
                        </Link>
                        <div className="hidden md:flex gap-x-md text-names-neutral-70 text-body-md">
                            {ROUTES.map((route) => (
                                <Link
                                    key={route.path}
                                    href={route.path}
                                    className="text-label-md hover:text-names-primary-80 md:whitespace-nowrap"
                                    data-testid={`${route.id}-link`}
                                    {...('isExternal' in route ? { target: '_blank' } : {})}
                                >
                                    {route.title}
                                </Link>
                            ))}
                        </div>
                    </div>
                    <div className="md:hidden">
                        <NavbarMobile routes={ROUTES}>
                            <ConnectButton />
                        </NavbarMobile>
                    </div>

                    {isAllowedSearchOnPage && (
                        <SearchInput isBelowMd onFocus={toggleSearchDialog} />
                    )}
                    <div className="hidden md:block">
                        <ConnectButton />
                    </div>
                </div>

                {isAllowedSearchOnPage && <SearchInput onFocus={toggleSearchDialog} />}
            </div>
        </nav>
    );
}

function SearchInput({ isBelowMd = false, onFocus }: { isBelowMd?: boolean; onFocus: () => void }) {
    const wrapperClass = isBelowMd
        ? 'md:flex hidden w-full max-w-[360px] justify-center mx-auto'
        : 'flex md:hidden w-full justify-center mx-auto';

    const containerClass =
        'w-full max-w-2xl flex flex-col backdrop-blur-md bg-white/5 overflow-hidden [&_*]:!border-none rounded-full default-ui-kit-styles';

    return (
        <div className={wrapperClass}>
            <div className={containerClass}>
                <Input
                    placeholder="Search for your IOTA name"
                    type={InputType.Text}
                    onFocus={onFocus}
                    trailingElement={<Search className="text-names-neutral-92 w-6 h-6" />}
                />
            </div>
        </div>
    );
}
