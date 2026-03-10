// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Close } from '@iota/apps-ui-icons';
import Link from 'next/link';
import React, { useEffect, useState } from 'react';

import { FOOTER_LEGAL_LINKS } from '@/lib/constants';
import {
    getAmplitudeConsentStatus,
    onAmplitudeConsentAccepted,
} from '@/lib/utils/analytics/amplitude';

const TEXT = 'By using this website, you agree with our ';

export function CookieDisclaimer() {
    const [amplitudeConsentStatus, setAmplitudeConsentStatus] = useState<
        'pending' | 'accepted' | 'declined' | null
    >(null);

    useEffect(() => {
        setAmplitudeConsentStatus(getAmplitudeConsentStatus());
    }, []);

    return (
        <>
            {amplitudeConsentStatus === 'pending' && (
                <div
                    className="fixed z-50 bg-names-neutral-6 max-w-none w-full flex flex-col !py-6 !px-8 max-md:space-y-4 max-md:px-0 bottom-0 right-0 rounded-none shadow-2xl md:flex-row md:space-x-4 md:py-4 md:px-6 md:bottom-6 md:right-6 md:rounded-md md:max-w-lg border border-transparent"
                    style={{
                        background:
                            'linear-gradient(#0a0d23, #0a0d23) padding-box, var(--names-gradient-primary) border-box',
                    }}
                >
                    <div className="flex max-md:container items-center justify-center md:items-start m-0 text-body-md !pl-0 [&_a]:text-names-primary-80 [&_a]:inline-flex">
                        <span className="text-names-neutral-92">
                            {TEXT}
                            {FOOTER_LEGAL_LINKS.map(({ title, path }, index) => {
                                return (
                                    <React.Fragment key={title}>
                                        <Link
                                            href={path}
                                            className="transition-colors duration-200"
                                        >
                                            {title}
                                        </Link>
                                        {index < FOOTER_LEGAL_LINKS.length - 1 ? ', ' : '.'}
                                    </React.Fragment>
                                );
                            })}
                        </span>
                    </div>
                    <div
                        onClick={() => {
                            onAmplitudeConsentAccepted();
                            setAmplitudeConsentStatus('accepted');
                        }}
                        className="absolute right-2 top-2 inline-flex cursor-pointer items-center justify-center rounded-full p-xs !mt-0 text-iota-neutral-100 outline-none hover:text-iota-neutral-92"
                    >
                        <Close className="h-5 w-5" aria-label="Close" />
                    </div>
                </div>
            )}
        </>
    );
}
