// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NetworkSelector } from '../network';
import { Search } from '../search';
import { LinkWithQuery } from '~/components/ui';
import { ThemedIotaLogo } from '~/components';
import { ThemeSwitcher } from '@iota/core';
import { useBreakpoint } from '~/hooks';
import clsx from 'clsx';

export function Header(): JSX.Element {
    const isSm = useBreakpoint('sm');
    return (
        <header
            className={clsx(
                `flex justify-center overflow-visible backdrop-blur-lg`,
                isSm ? 'h-header' : 'h-mobile-header flex-col gap-2 p-2',
            )}
        >
            <div className="container flex h-full flex-1 items-center justify-between gap-5">
                <LinkWithQuery
                    data-testid="nav-logo-button"
                    to="/"
                    className="flex flex-nowrap items-center gap-1 text-neutral-10"
                >
                    <ThemedIotaLogo />
                </LinkWithQuery>
                {isSm ? (
                    <div className="flex w-[360px] justify-center">
                        <Search />
                    </div>
                ) : null}
                <div className="flex flex-row gap-xs">
                    <ThemeSwitcher />
                    <NetworkSelector />
                </div>
            </div>
            {!isSm ? (
                <div className="flex justify-center">
                    <div className="flex w-[320px] justify-center">
                        <Search />
                    </div>
                </div>
            ) : null}
        </header>
    );
}
