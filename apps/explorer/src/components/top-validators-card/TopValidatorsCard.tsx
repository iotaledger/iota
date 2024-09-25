// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';

import { Banner, PlaceholderTable, TableCard } from '~/components/ui';
import { generateValidatorsTableColumns } from '~/lib/ui';
import { Panel, Title } from '@iota/apps-ui-kit';
import { ErrorBoundary } from '../error-boundary/ErrorBoundary';

const NUMBER_OF_VALIDATORS = 10;

type TopValidatorsCardProps = {
    limit?: number;
    showIcon?: boolean;
};

export function TopValidatorsCard({ limit, showIcon }: TopValidatorsCardProps): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');

    const tableColumns = generateValidatorsTableColumns({
        atRiskValidators: [],
        validatorEvents: [],
        rollingAverageApys: null,
        limit,
        showValidatorIcon: showIcon,
        includeColumns: ['Name', 'Address', 'Stake'],
    });

    if (isError || (!isPending && !data.activeValidators.length)) {
        return (
            <Banner variant="error" fullWidth>
                Validator data could not be loaded
            </Banner>
        );
    }

    return (
        <Panel>
            <Title title="Top Valdidators" />

            <div className="p-md">
                {isPending && (
                    <PlaceholderTable
                        rowCount={limit || NUMBER_OF_VALIDATORS}
                        rowHeight="13px"
                        colHeadings={['Name', 'Address', 'Stake']}
                    />
                )}

                {isSuccess && (
                    <ErrorBoundary>
                        <TableCard
                            data={data.activeValidators}
                            columns={tableColumns}
                            viewAll="/validators"
                            totalLabel={data ? data.activeValidators.length : '--'}
                        />
                    </ErrorBoundary>
                )}
            </div>
        </Panel>
    );
}
