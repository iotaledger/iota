// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';

import { Network } from '@iota/iota-sdk/client';
import {
    AddressesCardGraph,
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
import { useNetwork } from '~/hooks';

const TRANSACTIONS_LIMIT = 15;

export function Home(): JSX.Element {
    const [network] = useNetwork();
    const isIotaTokenCardEnabled = network === Network.Mainnet;
    return (
        <PageLayout
            content={
                <>
                    <div
                        data-testid="home-page"
                        className={clsx(
                            'home-page-grid-container-top mb-4',
                            isIotaTokenCardEnabled && 'with-token',
                        )}
                    >
                        <div style={{ gridArea: 'network' }} className="flex grow overflow-hidden">
                            <OnTheNetwork />
                        </div>
                        <div className="flex grow" style={{ gridArea: 'epoch' }}>
                            <CurrentEpoch />
                        </div>
                        {isIotaTokenCardEnabled ? (
                            <div style={{ gridArea: 'token' }}>
                                <IotaTokenCard />
                            </div>
                        ) : null}
                        <div className="flex grow" style={{ gridArea: 'transactions' }}>
                            <TransactionsCardGraph />
                        </div>
                        <div className="flex grow" style={{ gridArea: 'addresses' }}>
                            <AddressesCardGraph />
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
                                <TopValidatorsCard limit={10} showIcon />
                            </div>
                        </div>
                    </div>
                </>
            }
        />
    );
}
