// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider } from '@iota/apps-ui-kit';
import { LegalLinks, LegalText } from './Legal';
import { Link } from '~/components/ui';
import { FOOTER_LINKS } from '~/lib/constants';
import { ThemedIotaLogo } from '../ThemedIotaLogo';

function FooterLinks(): JSX.Element {
    return (
        <div className="flex flex-col items-center justify-center gap-6 md:flex-row md:justify-end">
            <ul className="flex flex-wrap gap-4 md:flex-row md:gap-6">
                {FOOTER_LINKS.map(({ title, href }) => (
                    <li key={href}>
                        <Link
                            variant="text"
                            href={href}
                            className="text-body-md text-neutral-40 dark:text-neutral-60"
                        >
                            {title}
                        </Link>
                    </li>
                ))}
            </ul>
        </div>
    );
}

export function Footer(): JSX.Element {
    return (
        <footer className="sticky top-[100%] bg-neutral-96 px-5 py-10 md:px-10 md:py-14 dark:bg-neutral-10">
            <nav className="container flex flex-col justify-center gap-md md:gap-lg">
                <div className="gap-7.5 flex flex-col-reverse items-center md:flex-row md:justify-between ">
                    <div className="hidden self-center md:flex md:self-start">
                        <ThemedIotaLogo />
                    </div>
                    <div>
                        <FooterLinks />
                    </div>
                </div>
                <Divider />
                <div className="flex flex-col-reverse justify-center gap-3 pt-3 md:flex-row md:justify-between">
                    <LegalText />
                    <LegalLinks />
                </div>
            </nav>
            <div className="mt-4 flex justify-center pt-5 md:hidden md:self-start">
                <ThemedIotaLogo />
            </div>
            <p className="mt-8 w-full text-center text-body-sm text-neutral-40 dark:text-neutral-60">
                {EXPLORER_REV}
            </p>
        </footer>
    );
}
