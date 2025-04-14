// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Link, PlaceholderTable, TableCard } from '~/components/ui';
import { generateValidatorsTableColumns } from '~/lib/ui';
import {
    Button,
    ButtonSize,
    ButtonType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Panel,
    Title,
} from '@iota/apps-ui-kit';
import { ErrorBoundary } from '../error-boundary/ErrorBoundary';
import { Warning } from '@iota/apps-ui-icons';
import { useIotaClientQuery } from '@iota/dapp-kit';

const NUMBER_OF_VALIDATORS = 10;

type TopValidatorsCardProps = {
    limit?: number;
    showIcon?: boolean;
};

export function TopValidatorsCard({ limit, showIcon }: TopValidatorsCardProps): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');

    const topCommitteeMembers =
        data?.committeeMembers.slice(0, limit || NUMBER_OF_VALIDATORS) ?? [];

    const tableColumns = generateValidatorsTableColumns({
        limit,
        showValidatorIcon: showIcon,
        includeColumns: ['Name', 'Address', 'Stake'],
    });

    if (isError || (!isPending && !data.committeeMembers.length)) {
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
            <div className="relative">
                <div className="absolute right-0 mr-4 mt-2">
                    <Link to="/validators">
                        <Button
                            type={ButtonType.Secondary}
                            size={ButtonSize.Small}
                            text="View All"
                        />
                    </Link>
                </div>
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
                                sortTable
                                defaultSorting={[{ id: 'stakingPoolIotaBalance', desc: true }]}
                                data={topCommitteeMembers}
                                columns={tableColumns}
                            />
                        </ErrorBoundary>
                    )}
                </div>
            </div>
        </Panel>
    );
}
