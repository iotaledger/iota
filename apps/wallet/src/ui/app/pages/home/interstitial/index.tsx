// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import ExternalLink from '_src/ui/app/components/external-link';
import { X32 } from '@iota/icons';
import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

import { Portal } from '../../../shared/Portal';

export type InterstitialConfig = {
    enabled: boolean;
    dismissKey?: string;
    imageUrl?: string;
    bannerUrl?: string;
};

interface InterstitialProps extends InterstitialConfig {
    onClose: () => void;
}

const setInterstitialDismissed = (dismissKey: string) => localStorage.setItem(dismissKey, 'true');

function Interstitial({ enabled, dismissKey, imageUrl, bannerUrl, onClose }: InterstitialProps) {
    const navigate = useNavigate();

    useEffect(() => {
        const t = setTimeout(setInterstitialDismissed, 1000);
        return () => clearTimeout(t);
    }, []);

    const closeInterstitial = (dismissKey?: string) => {
        if (dismissKey) {
            setInterstitialDismissed(dismissKey);
        }
        onClose();
        navigate('/apps');
    };

    if (!enabled) {
        return null;
    }

    return (
        <Portal containerId="overlay-portal-container">
            <div className="absolute bottom-0 left-0 right-0 top-0 z-50 flex flex-col flex-nowrap items-center justify-center overflow-hidden rounded-lg backdrop-blur-sm">
                {bannerUrl && (
                    <ExternalLink
                        href={bannerUrl}
                        onClick={() => {
                            ampli.clickedBullsharkQuestsCta({ sourceFlow: 'Interstitial' });
                            closeInterstitial();
                        }}
                        className="h-full w-full"
                    >
                        <img src={imageUrl} alt="interstitial-banner" />
                    </ExternalLink>
                )}
                <button
                    data-testid="bullshark-dismiss"
                    className="absolute bottom-0 w-full cursor-pointer appearance-none border-none bg-transparent pb-5"
                    onClick={() => closeInterstitial(dismissKey)}
                >
                    <X32 className="h-8 w-8 text-black" />
                </button>
            </div>
        </Portal>
    );
}

export default Interstitial;
