// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { LoadingIndicator, RadioGroup, RadioGroupItem } from '@iota/ui';
import { useState } from 'react';
import { type Direction } from 'react-resizable-panels';

import { ErrorBoundary, PkgModulesWrapper, TransactionBlocksForAddress } from '~/components';
import {
    AddressLink,
    ObjectLink,
    TabHeader,
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger,
} from '~/components/ui';
import { getOwnerStr, trimStdLibPrefix } from '~/lib/utils';
import { type DataType } from '../ObjectResultType';

import { ObjectFilterValue } from '~/lib/enums';
import styles from './ObjectView.module.css';

const GENESIS_TX_DIGEST = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=';

const SPLIT_PANELS_ORIENTATION: { label: string; value: Direction }[] = [
    { label: 'STACKED', value: 'vertical' },
    { label: 'SIDE-BY-SIDE', value: 'horizontal' },
];

interface PkgViewProps {
    data: DataType;
}

function PkgView({ data }: PkgViewProps): JSX.Element {
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
            <div>
                <TabHeader title="Details">
                    <table className={styles.description} id="descriptionResults">
                        <tbody>
                            <tr>
                                <td>Object ID</td>
                                <td id="objectID" className={styles.objectid}>
                                    <ObjectLink objectId={viewedData.id} noTruncate />
                                </td>
                            </tr>

                            <tr>
                                <td>Version</td>
                                <td>{viewedData.version}</td>
                            </tr>

                            {viewedData?.publisherAddress && (
                                <tr>
                                    <td>Publisher</td>
                                    <td id="lasttxID">
                                        <AddressLink
                                            address={viewedData.publisherAddress}
                                            noTruncate
                                        />
                                    </td>
                                </tr>
                            )}
                        </tbody>
                    </table>
                </TabHeader>

                <Tabs defaultValue="modules">
                    <TabsList>
                        <div className="mt-16 flex w-full justify-between">
                            <TabsTrigger value="modules">Modules</TabsTrigger>
                            <div className="hidden md:block">
                                <RadioGroup
                                    aria-label="split-panel-bytecode-viewer"
                                    value={selectedSplitPanelOrientation}
                                    onValueChange={(value) =>
                                        setSplitPanelOrientation(value as 'vertical' | 'horizontal')
                                    }
                                >
                                    {SPLIT_PANELS_ORIENTATION.map(({ value, label }) => (
                                        <RadioGroupItem key={value} value={value} label={label} />
                                    ))}
                                </RadioGroup>
                            </div>
                        </div>
                    </TabsList>
                    <TabsContent value="modules" noGap>
                        <ErrorBoundary>
                            <PkgModulesWrapper
                                id={data.id}
                                modules={properties}
                                splitPanelOrientation={selectedSplitPanelOrientation}
                            />
                        </ErrorBoundary>
                    </TabsContent>
                </Tabs>

                <div className={styles.txsection}>
                    <ErrorBoundary>
                        <TransactionBlocksForAddress
                            address={viewedData.id}
                            filter={ObjectFilterValue.Input}
                            header="Transaction Blocks"
                        />
                    </ErrorBoundary>
                </div>
            </div>
        </div>
    );
}

export default PkgView;
