// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaLogoWeb } from '@iota/ui-icons';

import { NetworkSelector } from '../network';
import Search from '../search/Search';
import { LinkWithQuery } from '~/components/ui';

function Header(): JSX.Element {
    return (
        <header className="flex h-header justify-center overflow-visible bg-neutral-98">
            <div className="container flex h-full flex-1 items-center justify-between gap-5">
                <LinkWithQuery
                    data-testid="nav-logo-button"
                    to="/"
                    className="flex flex-nowrap items-center gap-1 text-hero-darkest"
                >
                    <IotaLogoWeb width={137} height={36} />
                </LinkWithQuery>
                <div className="flex w-[360px] justify-center">
                    <Search />
                </div>
                <NetworkSelector />
            </div>
        </header>
    );
}

export default Header;
