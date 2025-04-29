// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Placeholder } from '@iota/apps-ui-kit';
import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { TableCard } from '~/components/ui';
import { generateObjectListColumns } from '~/lib/ui/utils/generateObjectListColumns';

interface ListViewProps {
    data?: IotaObjectResponse[];
    loading?: boolean;
}

export function ListView({ data, loading }: ListViewProps): JSX.Element {
    const tableColumns = generateObjectListColumns();

    return (
        <div className="h-full w-full">
            {tableColumns && data && <TableCard data={data ?? []} columns={tableColumns} />}
            {loading && new Array(10).fill(0).map((_, index) => <Placeholder key={index} />)}
        </div>
    );
}
