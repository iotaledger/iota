// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import Link from 'next/link';

import { NamesLogoBranded } from '@/components/svgs';
import { FOOTER_LEGAL_LINKS, FOOTER_SOCIAL_LINKS } from '@/lib/constants';

export function Footer() {
    const COPYRIGHT_YEAR = new Date().getFullYear();

    return (
        <footer className="w-full bg-names-neutral-6 py-lg">
            <div className="container flex flex-col items-center gap-y-lg">
                <div className="flex flex-col sm:flex-row justify-between items-center gap-y-lg w-full">
                    <div className="flex items-center gap-sm">
                        <NamesLogoBranded className="w-[14.71px] h-[22.97px]" />
                        <span className="text-names-neutral-70 text-label-sm tracking-normal">
                            © {COPYRIGHT_YEAR} IOTA Names. All Rights Reserved.
                        </span>
                    </div>
                    <div className="flex flex-row gap-md items-center text-names-neutral-70 text-body-md">
                        {FOOTER_LEGAL_LINKS.map(({ path, title }) => (
                            <Link
                                key={title}
                                href={path}
                                className="hover:text-names-primary-80 transition-colors duration-200"
                            >
                                {title}
                            </Link>
                        ))}
                        {FOOTER_SOCIAL_LINKS.map(({ path, icon, title }) => (
                            <div key={path} className="[&_svg]:h-6 [&_svg]:w-6">
                                <Link
                                    href={path}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    aria-label={title}
                                >
                                    {icon}
                                </Link>
                            </div>
                        ))}
                    </div>
                </div>
                <p className="w-full text-center text-label-sm text-names-neutral-70">
                    {process.env.NEXT_PUBLIC_IOTA_NAMES_REV}
                </p>
            </div>
        </footer>
    );
}
