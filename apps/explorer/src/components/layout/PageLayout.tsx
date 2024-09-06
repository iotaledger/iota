// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureIsOn } from '@growthbook/growthbook-react';
import { useAppsBackend } from '@iota/core';
import { LoadingIndicator } from '@iota/ui';
import { Network } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { type ReactNode, useRef } from 'react';

import Footer from '../footer/Footer';
import Header from '../header/Header';
import { useNetworkContext } from '~/contexts';
import { Banner } from '~/components/ui';

type PageLayoutProps = {
    content: ReactNode;
    loading?: boolean;
};

export function PageLayout({ content, loading }: PageLayoutProps): JSX.Element {
    const [network] = useNetworkContext();
    const { request } = useAppsBackend();
    const outageOverride = useFeatureIsOn('network-outage-override');

    const { data } = useQuery({
        queryKey: ['apps-backend', 'monitor-network'],
        queryFn: () =>
            request<{ degraded: boolean }>('monitor-network', {
                project: 'EXPLORER',
            }),
        // Keep cached for 2 minutes:
        staleTime: 2 * 60 * 1000,
        retry: false,
        enabled: network === Network.Mainnet,
    });
    const renderNetworkDegradeBanner =
        outageOverride || (network === Network.Mainnet && data?.degraded);
    const headerRef = useRef<HTMLElement | null>(null);

    const networkDegradeBannerCopy =
        network === Network.Testnet
            ? 'Iota Explorer (Testnet) is currently under-going maintenance. Some data may be incorrect or missing.'
            : "The explorer is running slower than usual. We're working to fix the issue and appreciate your patience.";

    return (
        <div className="relative min-h-screen w-full">
            <section ref={headerRef} className="fixed top-0 z-20 flex w-full flex-col">
                {renderNetworkDegradeBanner && (
                    <Banner rounded="none" align="center" variant="warning" fullWidth>
                        <div className="break-normal">{networkDegradeBannerCopy}</div>
                    </Banner>
                )}
                <Header />
            </section>
            {loading && (
                <div className="absolute left-1/2 right-0 top-1/2 flex -translate-x-1/2 -translate-y-1/2 transform justify-center">
                    <LoadingIndicator variant="lg" />
                </div>
            )}
            <main className="relative z-10 bg-neutral-98">
                {!loading && (
                    <section className="container pb-20 pt-28">
                        {content}
                    </section>
                )}
            </main>
            <Footer />
        </div>
    );
}
