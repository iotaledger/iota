// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator } from '@iota/ui';
import { useRef, type ReactNode } from 'react';

import Footer from '../footer/Footer';
import Header from '../header/Header';

type PageLayoutProps = {
    content: ReactNode;
    loading?: boolean;
};

export function PageLayout({ content, loading }: PageLayoutProps): JSX.Element {
    const headerRef = useRef<HTMLElement | null>(null);

    return (
        <div className="relative min-h-screen w-full">
            <section ref={headerRef} className="fixed top-0 z-20 flex w-full flex-col">
                <Header />
            </section>
            {loading && (
                <div className="absolute left-1/2 right-0 top-1/2 flex -translate-x-1/2 -translate-y-1/2 transform justify-center">
                    <LoadingIndicator variant="lg" />
                </div>
            )}
            <main className="relative z-10 bg-neutral-98">
                {!loading && <section className="container pb-20 pt-28">{content}</section>}
            </main>
            <Footer />
        </div>
    );
}
