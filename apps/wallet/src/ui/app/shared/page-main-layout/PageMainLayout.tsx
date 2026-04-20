// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, MenuContent, Navigation } from '_components';
import cn from 'clsx';
import { createContext, type ReactNode } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { useLocation } from 'react-router-dom';
import { Toaster } from '../toaster';

export const PageMainLayoutContext = createContext<HTMLDivElement | null>(null);

export interface PageMainLayoutProps {
    children: ReactNode | ReactNode[];
    bottomNavEnabled?: boolean;
    topNavMenuEnabled?: boolean;
    dappStatusEnabled?: boolean;
}

function RouteTransition({ routeKey, children }: { routeKey: string; children: ReactNode }) {
    return (
        <AnimatePresence mode="wait" initial={false}>
            <motion.div
                key={routeKey}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0, transition: { duration: 0.08 } }}
                transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
                className="flex h-full w-full flex-col"
            >
                {children}
            </motion.div>
        </AnimatePresence>
    );
}

export function PageMainLayout({
    children,
    bottomNavEnabled = false,
    topNavMenuEnabled = false,
}: PageMainLayoutProps) {
    const location = useLocation();
    const topLevelRoute = location.pathname.split('/')[1] ?? '';

    return (
        <div className="flex h-full max-h-full w-full flex-1 flex-col flex-nowrap items-stretch overflow-hidden rounded-xl">
            <div className="flex w-full flex-1 flex-row overflow-hidden">
                {bottomNavEnabled && <Navigation />}
                <div
                    className={cn(
                        'flex flex-1 flex-col flex-nowrap overflow-hidden',
                        bottomNavEnabled ? 'px-md py-sm' : '',
                    )}
                >
                    <div className="relative flex flex-grow flex-col flex-nowrap overflow-hidden">
                        <div id="overlay-portal-container" />
                        <div className="flex h-full flex-col overflow-hidden bg-iota-neutral-100 dark:bg-iota-neutral-6">
                            <main className="flex h-full w-full flex-col overflow-hidden">
                                <RouteTransition routeKey={topLevelRoute}>
                                    <PageMainLayoutContext.Provider value={null}>
                                        <ErrorBoundary>{children}</ErrorBoundary>
                                    </PageMainLayoutContext.Provider>
                                </RouteTransition>
                            </main>
                            <Toaster bottomNavEnabled={bottomNavEnabled} />
                        </div>
                        {topNavMenuEnabled ? <MenuContent /> : null}
                    </div>
                </div>
            </div>
        </div>
    );
}
