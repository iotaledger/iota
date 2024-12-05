// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NetworkSelector } from '../network';
import Search from '../search/Search';
import { LinkWithQuery } from '~/components/ui';
import { ThemeSwitcher, ThemedIotaLogo } from '~/components';

function Header(): JSX.Element {
    return (
        <header className="flex h-header justify-center overflow-visible backdrop-blur-lg">
            <div className="container flex h-full flex-1 items-center justify-between gap-5">
                <LinkWithQuery
                    data-testid="nav-logo-button"
                    to="/"
                    className="flex flex-nowrap items-center gap-1 text-neutral-10"
                >
                    <ThemedIotaLogo />
                </LinkWithQuery>
                <div className="flex w-[360px] justify-center">
                    <Search />
                </div>
                <div className="flex flex-row gap-xs">
                    <ThemeSwitcher />
                    <NetworkSelector />
                </div>
            </div>
        </header>
    );
}

export default Header;
