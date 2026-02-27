// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaLogoWeb } from '@iota/apps-ui-icons';
import { Divider } from '@iota/apps-ui-kit';
import { Link } from '../link';
import { LEGAL_LINKS } from '../../lib/constants/routes.constants';
import { ExternalLink } from '../../lib/types/link';

const EXTERNAL_LINKS: ExternalLink[] = [
    {
        text: 'Discord',
        url: 'https://discord.iota.org/',
        isExternal: true,
    },
    {
        text: 'LinkedIn',
        url: 'https://www.linkedin.com/company/iotafoundation/',
        isExternal: true,
    },
    {
        text: 'Twitter',
        url: 'https://twitter.com/iota',
        isExternal: true,
    },
    {
        text: 'GitHub',
        url: 'https://www.github.com/iotaledger/',
        isExternal: true,
    },
    {
        text: 'Youtube',
        url: 'https://www.youtube.com/c/iotafoundation',
        isExternal: true,
    },
];

export function Footer() {
    return (
        <footer className="w-full  dark:bg-iota-neutral-10 bg-iota-neutral-92 pt-lg pb-2xl">
            <div className="container flex flex-col justify-center gap-y-lg md:gap-md--rs">
                <div className="flex flex-col md:flex-row justify-between items-center gap-y-lg">
                    <IotaLogoWeb className="w-36 h-9 dark:text-iota-neutral-92 text-iota-neutral-10" />
                    <div className="flex flex-row gap-lg items-center">
                        {EXTERNAL_LINKS.map(({ url, text, isExternal }) => (
                            <Link key={text} href={url} isSecondary isExternal={isExternal}>
                                {text}
                            </Link>
                        ))}
                    </div>
                </div>

                <Divider />

                <div className="flex flex-col-reverse md:flex-row justify-between items-center gap-y-lg">
                    <span className="text-iota-neutral-40 dark:text-iota-neutral-60 text-body-md tracking-normal">
                        © {new Date().getFullYear()} IOTA Foundation. All Rights Reserved.
                    </span>

                    <div className="flex flex-row gap-lg items-center">
                        {LEGAL_LINKS.map(({ url, text, isExternal }) => (
                            <Link key={text} href={url} isSecondary isExternal={isExternal}>
                                {text}
                            </Link>
                        ))}
                    </div>
                </div>

                <span className="w-full text-center text-iota-neutral-40 dark:text-iota-neutral-60 text-label-md">
                    {COMMIT_REV}
                </span>
            </div>
        </footer>
    );
}
