// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { MenuIcon } from '@iota/apps-ui-icons';
import Link from 'next/link';
import { useState } from 'react';

import { NamesLogoWeb } from '@/components/svgs';
import type { Route } from '@/lib/interfaces';

import { NavbarMobileContainer } from './NavbarMobileContainer';

type NavbarMobileProps = {
    routes: Route[];
    children?: React.ReactNode;
};

export function NavbarMobile({ routes, children }: NavbarMobileProps) {
    const [open, setOpen] = useState(false);

    function handleOpen() {
        document.documentElement.classList.add('overflow-hidden');
        setOpen(true);
    }

    function handleClose() {
        document.documentElement.classList.remove('overflow-hidden');
        setOpen(false);
    }
    return (
        <>
            <button
                type="button"
                className="w-6 h-6 [&_svg]:w-full [&_svg]:h-full"
                onClick={handleOpen}
                aria-label="Open menu"
            >
                <MenuIcon />
            </button>

            <NavbarMobileContainer
                routes={routes}
                onClose={handleClose}
                logo={
                    <Link href="/" aria-label="Go to homepage">
                        <NamesLogoWeb className="w-32 h-2xl text-names-primary-100" />
                    </Link>
                }
                isOpen={open}
            >
                {children}
            </NavbarMobileContainer>
        </>
    );
}
