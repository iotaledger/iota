// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider } from '@iota/apps-ui-kit';
import { LegalLinks, LegalText } from './Legal';
import { IotaLogoWeb } from '@iota/ui-icons';
import { Link } from '~/components/ui';
import { FOOTER_LINKS } from '~/lib/constants';
import { rev } from '~/../rev.json'

function FooterLinks(): JSX.Element {
    return (
        <div className="flex flex-col items-center justify-center gap-6 md:flex-row md:justify-end">
            <ul className="flex flex-wrap gap-4 md:flex-row md:gap-6">
                {FOOTER_LINKS.map(({ title, href }) => (
                    <li key={href}>
                        <Link variant="text" href={href} className="text-body-md text-neutral-40">
                            {title}
                        </Link>
                    </li>
                ))}
            </ul>
        </div>
    );
}

function Footer(): JSX.Element {
    return (
        <footer className="sticky top-[100%] bg-neutral-96 px-5 py-10 md:px-10 md:py-14">
            <nav className="container flex flex-col justify-center gap-md md:gap-lg">
                <div className="flex flex-col-reverse items-center gap-7.5 md:flex-row md:justify-between ">
                    <div className="hidden self-center text-neutral-10 md:flex md:self-start">
                        <IotaLogoWeb width={137} height={36} />
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
            <div className="mt-4 flex justify-center pt-5 text-neutral-10 md:hidden md:self-start">
                <IotaLogoWeb width={137} height={36} />
            </div>
            <p className="text-neutral-40 text-center w-full p-8">{rev}</p>
        </footer>
    );
}

export default Footer;
