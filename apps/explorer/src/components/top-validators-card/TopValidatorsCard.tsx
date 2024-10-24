// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { PlaceholderTable, TableCard } from '~/components/ui';
import { generateValidatorsTableColumns } from '~/lib/ui';
import { InfoBox, InfoBoxStyle, InfoBoxType, Panel, Title } from '@iota/apps-ui-kit';
import { ErrorBoundary } from '../error-boundary/ErrorBoundary';
import { Warning } from '@iota/ui-icons';

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
            <InfoBox
                title="Failed loading data"
                supportingText="Validator data could not be loaded"
                icon={<Warning />}
                type={InfoBoxType.Error}
                style={InfoBoxStyle.Elevated}
            />
        );
    }

    return (
        <Panel>
            <Title title="Top Validators" />

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
                            totalLabel={data ? data.activeValidators.length.toString() : '--'}
                        />
                    </ErrorBoundary>
                )}
            </div>
        </Panel>
    );
}
