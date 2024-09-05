// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaLogoWeb } from '@iota/ui-icons';

import { NetworkSelector } from '../network';
import Search from '../search/Search';
import { LinkWithQuery } from '~/components/ui';

function Header(): JSX.Element {
    return (
        <header className="flex h-header justify-center overflow-visible bg-white">
            <div className="2xl:p-0 flex h-full max-w-[1440px] flex-1 items-center gap-5 px-4 py-8 lg:px-6 xl:px-10">
                <LinkWithQuery
                    data-testid="nav-logo-button"
                    to="/"
                    className="flex flex-nowrap items-center gap-1 text-hero-darkest"
                >
                    <IotaLogoWeb width={137} height={36} />
                </LinkWithQuery>
                <div className="flex w-full gap-2">
                    <div className="flex flex-1 justify-center">
                        <Search />
                    </div>
                    <NetworkSelector />
                </div>
            </div>
        </header>
    );
}

export default Header;
