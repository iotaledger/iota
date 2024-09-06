// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '@iota/ui';

import { LegalLinks, LegalText } from './Legal';
import { IotaLogoWeb } from '@iota/ui-icons';
import { Link } from '~/components/ui';
import { FOOTER_LINKS } from '~/lib/constants';

function FooterLinks(): JSX.Element {
    return (
        <div className="flex flex-col items-center justify-center gap-6 md:flex-row md:justify-end">
            <ul className="flex gap-4 md:flex-row md:gap-6">
                {FOOTER_LINKS.map(({ title, href }) => (
                    <li key={href}>
                        <Link variant="text" href={href}>
                            <Text variant="body/medium" color="steel-darker">
                                {title}
                            </Text>
                        </Link>
                    </li>
                ))}
            </ul>
        </div>
    );
}

function Footer(): JSX.Element {
    return (
        <footer className="sticky top-[100%] bg-gray-40 px-5 py-10 md:px-10 md:py-14">
            <nav className="flex flex-col justify-center gap-4 divide-y divide-solid divide-gray-45 md:gap-7.5">
                <div className="flex flex-col-reverse items-center gap-7.5 md:flex-row md:justify-between ">
                    <div className="hidden self-center text-hero-dark md:flex md:self-start">
                        <IotaLogoWeb width={137} height={36} />
                    </div>
                    <div>
                        <FooterLinks />
                    </div>
                </div>
                <div className="flex flex-col-reverse justify-center gap-3 pt-3 md:flex-row md:justify-between">
                    <LegalText />
                    <LegalLinks />
                </div>
            </nav>
            <div className="mt-4 flex justify-center border-t border-solid border-gray-45 pt-5 text-hero-dark md:hidden md:self-start">
                <IotaLogoWeb width={137} height={36} />
            </div>
        </footer>
    );
}

export default Footer;
