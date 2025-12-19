// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Close } from '@iota/apps-ui-icons';
import React, { useEffect, useState } from 'react';

import { getAmplitudeConsentStatus } from './helpers';

interface DisclaimerProps {
    onClose: () => void;
    children: React.ReactNode;
}

export function Disclaimer({ onClose, children }: DisclaimerProps) {
    // Internal status to hide component on close
    const [amplitudeConsentStatus, setAmplitudeConsentStatus] = useState<
        'pending' | 'accepted' | 'declined' | null
    >(null);

    useEffect(() => {
        setAmplitudeConsentStatus(getAmplitudeConsentStatus());
    }, []);

    return (
        <>
            {amplitudeConsentStatus === 'pending' && (
                <div className=" fixed bottom-0 dark:border-iota-neutral-12 dark:bg-iota-neutral-20 border-iota-neutral-80 bg-iota-neutral-100 right-0 z-[99999] w-full max-w-none flex flex-col py-6 px-8 rounded-none max-lg:px-0 lg:flex-row lg:py-4 lg:px-6 lg:pr-10 lg:bottom-6 lg:right-6 lg:rounded-md lg:max-w-[32rem] border">
                    <div
                        onClick={() => {
                            setAmplitudeConsentStatus('accepted');
                            onClose();
                        }}
                        className="absolute right-2 top-2 inline-flex cursor-pointer items-center justify-center rounded-full p-xs !mt-0 text-iota-neutral-10 dark:text-iota-neutral-92 outline-none hover:opacity-70"
                    >
                        <Close className="h-5 w-5" aria-label="Close" />
                    </div>
                    <div className="flex flex-col items-center justify-center lg:items-start m-0 text-body-md text-iota-neutral-10 dark:text-iota-neutral-92">
                        {children}
                    </div>
                </div>
            )}
        </>
    );
}
