// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { ExternalLink } from '_components';
import { useEffect } from 'react';
import { Portal } from '../../../shared/Portal';
import { Close } from '@iota/apps-ui-icons';
import { Button, ButtonType } from '@iota/apps-ui-kit';

export type InterstitialConfig = {
    enabled: boolean;
    dismissKey?: string;
    imageUrl?: string;
    bannerUrl?: string;
    minVersion?: string;
};

interface InterstitialProps extends InterstitialConfig {
    onClose: () => void;
}

const setInterstitialDismissed = (dismissKey: string) => localStorage.setItem(dismissKey, 'true');
const getValidDismissKey = (dismissKey?: string) => {
    const normalizedDismissKey = dismissKey?.trim();
    return normalizedDismissKey && normalizedDismissKey !== 'undefined'
        ? normalizedDismissKey
        : null;
};

export function Interstitial({
    enabled,
    dismissKey,
    imageUrl,
    bannerUrl,
    onClose,
}: InterstitialProps) {
    const overlayContainer = document.getElementById('overlay-portal-container');
    const validDismissKey = getValidDismissKey(dismissKey);

    useEffect(() => {
        if (!validDismissKey) {
            return;
        }

        const t = setTimeout(() => setInterstitialDismissed(validDismissKey), 1000);
        return () => clearTimeout(t);
    }, [validDismissKey]);

    const closeInterstitial = (dismissKey?: string) => {
        const validDismissKey = getValidDismissKey(dismissKey);
        if (validDismissKey) {
            setInterstitialDismissed(validDismissKey);
        }
        onClose();
    };

    // Prevent crash: if the portal container is not mounted yet, do not render the interstitial
    if (!enabled || !overlayContainer) {
        return null;
    }

    if (!imageUrl) return null;

    const imageNode = (
        <img src={imageUrl} alt="interstitial-banner" className="h-full w-auto max-w-full" />
    );

    return (
        <Portal containerId="overlay-portal-container">
            <div className="absolute inset-0 z-50 flex items-center justify-center p-5 backdrop-blur-sm">
                <div className="relative h-full overflow-hidden rounded-3xl">
                    {bannerUrl ? (
                        <ExternalLink
                            href={bannerUrl}
                            onClick={() => {
                                ampli.clickedAppsBannerCta({
                                    sourceFlow: 'Interstitial',
                                    bannerUrl,
                                });
                                closeInterstitial(dismissKey);
                            }}
                            className="block h-full"
                            trackEvent={false}
                        >
                            {imageNode}
                        </ExternalLink>
                    ) : (
                        imageNode
                    )}

                    <div className="absolute right-3 top-3">
                        <Button
                            type={ButtonType.Secondary}
                            onClick={() => closeInterstitial(dismissKey)}
                            icon={<Close className="h-4 w-4" />}
                            aria-label="Close"
                        />
                    </div>
                </div>
            </div>
        </Portal>
    );
}
