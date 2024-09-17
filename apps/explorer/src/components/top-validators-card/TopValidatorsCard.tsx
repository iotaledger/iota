// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { ArrowRight12 } from '@iota/icons';
import { Text } from '@iota/ui';
import { useMemo } from 'react';

import { Banner, Link, PlaceholderTable, TableCard } from '~/components/ui';
import { generateValidatorsTableColumns } from '~/lib/ui';

const NUMBER_OF_VALIDATORS = 10;

type TopValidatorsCardProps = {
    limit?: number;
    showIcon?: boolean;
};

export function TopValidatorsCard({ limit, showIcon }: TopValidatorsCardProps): JSX.Element {
    const { data, isPending, isSuccess, isError } = useIotaClientQuery('getLatestIotaSystemState');

    const tableColumns = useMemo(
        () =>
            data
                ? generateValidatorsTableColumns({
                      atRiskValidators: [],
                      validatorEvents: [],
                      rollingAverageApys: null,
                      limit,
                      showValidatorIcon: showIcon,
                      filterColumns: ['Name', 'Address', 'Stake'],
                  })
                : null,
        [data, limit, showIcon],
    );

    if (isError || (!isPending && !data.activeValidators.length)) {
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

            {isSuccess && (
                <>
                    <TableCard data={data.activeValidators} columns={tableColumns} />
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
