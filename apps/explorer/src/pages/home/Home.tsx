// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import { lazy, Suspense } from 'react';

import { Network } from '@iota/iota-sdk/client';
import {
    AccountsCardGraph,
    Activity,
    CurrentEpoch,
    ErrorBoundary,
    IotaTokenCard,
    OnTheNetwork,
    PageLayout,
    TopPackagesCard,
    TopValidatorsCard,
    TransactionsCardGraph,
} from '~/components';
import { Card, TabHeader } from '~/components/ui';
import { useNetwork } from '~/hooks';

const ValidatorMap = lazy(() => import('../../components/validator-map/ValidatorMap'));

const TRANSACTIONS_LIMIT = 25;

function Home(): JSX.Element {
    const [network] = useNetwork();
    const isIotaTokenCardEnabled = network === Network.Mainnet;
    return (
        <PageLayout>
            <div className="m-16">
                <div
                    data-testid="home-page"
                    className={clsx(
                        'home-page-grid-container-top mb-4',
                        isIotaTokenCardEnabled && 'with-token',
                    )}
                >
                    <div style={{ gridArea: 'network' }} className="overflow-hidden">
                        <OnTheNetwork />
                    </div>
                    <div style={{ gridArea: 'epoch' }}>
                        <CurrentEpoch />
                    </div>
                    {isIotaTokenCardEnabled ? (
                        <div style={{ gridArea: 'token' }}>
                            <IotaTokenCard />
                        </div>
                    ) : null}
                    <div style={{ gridArea: 'transactions' }}>
                        <TransactionsCardGraph />
                    </div>
                    <div style={{ gridArea: 'accounts' }}>
                        <AccountsCardGraph />
                    </div>
                </div>
                <div>
                    <div className="m-b-12" style={{ gridArea: 'activity' }}>
                        <ErrorBoundary>
                            <Activity initialLimit={TRANSACTIONS_LIMIT} disablePagination />
                        </ErrorBoundary>
                    </div>
                    <div className="home-page-grid-container-bottom">
                        <div className="m-b-12" style={{ gridArea: 'packages' }}>
                            <TopPackagesCard />
                        </div>
                        <div
                            className="m-b-12"
                            data-testid="validators-table"
                            style={{ gridArea: 'validators' }}
                        >
                            <TabHeader title="Validators">
                                <ErrorBoundary>
                                    <TopValidatorsCard limit={10} showIcon />
                                </ErrorBoundary>
                            </TabHeader>
                        </div>
                        <div
                            style={{ gridArea: 'node-map' }}
                            className="min-h-[320px] sm:min-h-[380px] lg:min-h-[460px] xl:min-h-[520px]"
                        >
                            <ErrorBoundary>
                                <Suspense fallback={<Card height="full" />}>
                                    <ValidatorMap minHeight="100%" />
                                </Suspense>
                            </ErrorBoundary>
                        </div>
                    </div>
                </div>
            </div>
        </PageLayout>
    );
}

export default Home;
