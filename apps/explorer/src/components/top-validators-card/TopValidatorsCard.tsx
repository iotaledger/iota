// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { ArrowRight12 } from '@iota/icons';
import { Text } from '@iota/ui';
import { useMemo } from 'react';

import { Banner, Link, PlaceholderTable, TableCard } from '~/components/ui';
import { generateValidatorsTableData } from '~/lib/ui';

const NUMBER_OF_VALIDATORS = 10;

const tableColumns = [
    {
        header: 'Name',
        accessorKey: 'name',
    },
    {
        header: 'Address',
        accessorKey: 'address',
    },
    {
        header: 'Stake',
        accessorKey: 'stake',
    },
];

type TopValidatorsCardProps = {
    limit?: number;
    showIcon?: boolean;
};

export function TopValidatorsCard({ limit, showIcon }: TopValidatorsCardProps): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');

    const tableData = useMemo(
        () =>
            data
                ? generateValidatorsTableData({
                      validators: data.activeValidators,
                      atRiskValidators: [],
                      validatorEvents: [],
                      rollingAverageApys: null,
                      limit,
                      showValidatorIcon: showIcon,
                      columns: tableColumns,
                  })
                : null,
        [data, limit, showIcon],
    );

    if (isError || (!isPending && !tableData?.data.length)) {
        return (
            <Banner variant="error" fullWidth>
                Validator data could not be loaded
            </Banner>
        );
    }

    return (
        <>
            {isPending && (
                <PlaceholderTable
                    rowCount={limit || NUMBER_OF_VALIDATORS}
                    rowHeight="13px"
                    colHeadings={['Name', 'Address', 'Stake']}
                />
            )}

            {isSuccess && tableData && (
                <>
                    <TableCard data={tableData.data} columns={tableData.columns} />
                    <div className="mt-3 flex justify-between">
                        <Link to="/validators">
                            <div className="flex items-center gap-2">
                                View all
                                <ArrowRight12 fill="currentColor" className="h-3 w-3 -rotate-45" />
                            </div>
                        </Link>
                        <Text variant="body/medium" color="steel-dark">
                            {data ? data.activeValidators.length : '-'}
                            {` Total`}
                        </Text>
                    </div>
                </>
            )}
        </>
    );
}
