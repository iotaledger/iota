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
            <ul className="flex flex-wrap justify-center gap-4 md:flex-row md:gap-6">
                {FOOTER_LINKS.map(({ title, href }) => (
                    <li key={href}>
                        <Link
                            variant="text"
                            href={href}
                            className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60"
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
        <footer className="sticky top-[100%] bg-iota-neutral-96 px-5 py-10 md:px-10 md:py-14 dark:bg-iota-neutral-10">
            <nav className="container flex flex-col justify-center gap-md md:gap-lg">
                <div className="gap-7.5 flex flex-col-reverse items-center md:flex-row md:justify-between ">
                    <div className="hidden self-center md:flex md:self-start">
                        <ThemedIotaLogo />
                    </div>
                    <FooterLinks />
                </div>
                <Divider />
                <div className="flex flex-col gap-y-8">
                    <div className="flex flex-col-reverse justify-center gap-3 pt-3 md:flex-row md:justify-between">
                        <LegalText />
                        <LegalLinks />
                    </div>
                    <div className="flex justify-center md:hidden md:self-start">
                        <ThemedIotaLogo />
                    </div>
                    <p className="w-full text-center text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                        {EXPLORER_REV}
                    </p>
                </div>
            </nav>
        </footer>
    );
}
