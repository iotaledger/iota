// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaLogoWeb } from '@iota/ui-icons';
import clsx from 'clsx';
import { useEffect, useState } from 'react';

import { NetworkSelector } from '../network';
import Search from '../search/Search';
import { LinkWithQuery } from '~/components/ui';

function Header(): JSX.Element {
    const [isScrolled, setIsScrolled] = useState(window.scrollY > 0);
    useEffect(() => {
        const callback = () => {
            setIsScrolled(window.scrollY > 0);
        };
        document.addEventListener('scroll', callback, { passive: true });
        return () => {
            document.removeEventListener('scroll', callback);
        };
    }, []);

    return (
        <header
            className={clsx(
                'flex h-header justify-center overflow-visible backdrop-blur-xl transition-shadow',
                isScrolled && 'shadow-effect-ui-regular',
            )}
        >
            <div className="2xl:p-0 flex h-full max-w-[1440px] flex-1 items-center gap-5 px-4 py-8 lg:px-6 xl:px-10">
                <LinkWithQuery
                    data-testid="nav-logo-button"
                    to="/"
                    className="flex flex-nowrap items-center gap-1 text-hero-darkest"
                >
                    <IotaLogoWeb width={130} height={32} />
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
