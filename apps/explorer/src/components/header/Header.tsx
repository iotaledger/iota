// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Iota, IotaLogoTxt } from '@iota/icons';
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
                'flex h-header justify-center overflow-visible bg-white/40 backdrop-blur-xl transition-shadow',
                isScrolled && 'shadow-effect-ui-regular',
            )}
        >
            <div className="2xl:p-0 flex h-full max-w-[1440px] flex-1 items-center gap-5 px-5">
                <LinkWithQuery
                    data-testid="nav-logo-button"
                    to="/"
                    className="flex flex-nowrap items-center gap-1 text-hero-darkest"
                >
                    <Iota className="h-[26px] w-5" />
                    <IotaLogoTxt className="h-[17px] w-[27px]" />
                </LinkWithQuery>
                <div className="flex w-full gap-2">
                    <div className="flex-1">
                        <Search />
                    </div>
                    <NetworkSelector />
                </div>
            </div>
        </header>
    );
}

export default Header;
