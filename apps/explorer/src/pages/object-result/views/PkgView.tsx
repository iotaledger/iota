// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { useState } from 'react';
import { type Direction } from 'react-resizable-panels';

import {
    AddressLink,
    ErrorBoundary,
    ObjectLink,
    PkgModulesWrapper,
    TransactionBlocksForAddress,
} from '~/components';
import { getOwnerStr, onCopySuccess, trimStdLibPrefix } from '~/lib/utils';
import { type DataType } from '../ObjectResultType';

import { ObjectFilterValue } from '~/lib/enums';
import {
    ButtonSegment,
    ButtonSegmentType,
    KeyValueInfo,
    LoadingIndicator,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Title,
} from '@iota/apps-ui-kit';

const GENESIS_TX_DIGEST = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=';

const SPLIT_PANELS_ORIENTATION: { label: string; value: Direction }[] = [
    { label: 'Stacked', value: 'vertical' },
    { label: 'Side-by-side', value: 'horizontal' },
];

interface PkgViewProps {
    data: DataType;
}

export function PkgView({ data }: PkgViewProps): JSX.Element {
    const [selectedSplitPanelOrientation, setSplitPanelOrientation] = useState(
        SPLIT_PANELS_ORIENTATION[1].value,
    );

    const { data: txnData, isPending } = useGetTransaction(data.data.tx_digest!);

    if (isPending) {
        return <LoadingIndicator text="Loading data" />;
    }
    const viewedData = {
        ...data,
        objType: trimStdLibPrefix(data.objType),
        tx_digest: data.data.tx_digest,
        owner: getOwnerStr(data.owner),
        publisherAddress:
            data.data.tx_digest === GENESIS_TX_DIGEST
                ? 'Genesis'
                : txnData?.transaction?.data.sender,
    };

    const filterProperties = (
        entry: [string, unknown],
    ): entry is [string, number] | [string, string] =>
        ['number', 'string'].includes(typeof entry[1]);

    const mapProperties = ([key, value]: [string, number] | [string, string]): [string, string] => [
        key,
        value.toString(),
    ];

    const properties = Object.entries(viewedData.data.contents ?? {})
        .filter(([key, _]) => key !== 'name')
        .filter(filterProperties)
        .map(mapProperties);

    return (
        <div>
            <div className="flex flex-col gap-2xl">
                <Panel>
                    <Title title="Details" />
                    <div className="flex flex-col gap-lg p-md--rs">
                        <KeyValueInfo
                            keyText="Object ID"
                            value={<ObjectLink objectId={viewedData.id} label={viewedData.id} />}
                            copyText={viewedData.id}
                            onCopySuccess={onCopySuccess}
                        />

                        <KeyValueInfo keyText="Version" value={viewedData.version} />
                        {viewedData?.publisherAddress && (
                            <KeyValueInfo
                                keyText="Publisher"
                                value={
                                    <AddressLink
                                        address={viewedData.publisherAddress}
                                        label={viewedData.publisherAddress}
                                    />
                                }
                                copyText={viewedData.publisherAddress}
                                onCopySuccess={onCopySuccess}
                            />
                        )}
                    </div>
                </Panel>

                <Panel>
                    <Title
                        title="Modules"
                        trailingElement={
                            <div className="hidden md:flex">
                                <SegmentedButton
                                    type={SegmentedButtonType.Outlined}
                                    shape={ButtonSegmentType.Rounded}
                                >
                                    {SPLIT_PANELS_ORIENTATION.map(({ value, label }) => (
                                        <ButtonSegment
                                            key={value}
                                            type={ButtonSegmentType.Rounded}
                                            onClick={() => setSplitPanelOrientation(value)}
                                            selected={selectedSplitPanelOrientation === value}
                                            label={label}
                                        />
                                    ))}
                                </SegmentedButton>
                            </div>
                        }
                    />
                    <div className="h-full p-md--rs">
                        <ErrorBoundary>
                            <PkgModulesWrapper
                                id={data.id}
                                modules={properties}
                                splitPanelOrientation={selectedSplitPanelOrientation}
                            />
                        </ErrorBoundary>
                    </div>
                </Panel>

                <ErrorBoundary>
                    <TransactionBlocksForAddress
                        address={viewedData.id}
                        filter={ObjectFilterValue.Input}
                        header="Transaction Blocks"
                    />
                </ErrorBoundary>
            </div>
        </div>
    );
}
