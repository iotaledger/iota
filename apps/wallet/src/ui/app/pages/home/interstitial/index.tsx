// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonOrLink } from '_app/shared/utils/ButtonOrLink';
import { ampli } from '_src/shared/analytics/ampli';
import ExternalLink from '_src/ui/app/components/external-link';
import { Text } from '_src/ui/app/shared/text';
import { X32 } from '@iota/icons';
import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

import { Portal } from '../../../shared/Portal';

export type InterstitialConfig = {
    enabled: boolean;
    dismissKey?: string;
    imageUrl?: string;
    buttonText?: string;
    bannerUrl?: string;
};

interface InterstitialProps extends InterstitialConfig {
    onClose: () => void;
}

const setInterstitialDismissed = (dismissKey: string) => localStorage.setItem(dismissKey, 'true');

function Interstitial({
    enabled,
    dismissKey,
    imageUrl,
    bannerUrl,
    buttonText,
    onClose,
}: InterstitialProps) {
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

    const onClick = () => {
        ampli.clickedBullsharkQuestsCta({ sourceFlow: 'Interstitial' });
        closeInterstitial();
    };

    if (!enabled) {
        return null;
    }

    return (
        <Portal containerId="overlay-portal-container">
            <div className="absolute bottom-0 left-0 right-0 top-0 z-50 flex h-full flex-col flex-nowrap items-center justify-between overflow-hidden rounded-lg bg-[rgba(17,35,55,.56)] py-8 backdrop-blur-sm">
                <button
                    data-testid="bullshark-dismiss"
                    className="w-full cursor-pointer appearance-none border-none bg-transparent"
                    onClick={() => closeInterstitial(dismissKey)}
                >
                    <X32 className="h-8 w-8 text-white" />
                </button>
                {bannerUrl && (
                    <ExternalLink href={bannerUrl} onClick={onClick} className="w-full text-center">
                        <img
                            className="w-full rounded-2xl p-2"
                            src={imageUrl}
                            alt="interstitial-banner"
                        />
                    </ExternalLink>
                )}
                <ButtonOrLink
                    className="flex h-10 cursor-pointer appearance-none items-center rounded-full border-none bg-[#4CA2FF] px-6 no-underline"
                    onClick={onClick}
                    to={bannerUrl}
                    target="_blank"
                    rel="noreferrer noopener"
                >
                    <Text variant="body" weight="semibold" color="white">
                        {buttonText || 'Join for a chance to win'}
                    </Text>
                </ButtonOrLink>
            </div>
        </Portal>
    );
}

export default Interstitial;
