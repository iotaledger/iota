// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator, RadioGroup, RadioGroupItem } from '@iota/ui';
import { useState } from 'react';
import { type Direction } from 'react-resizable-panels';

import { ErrorBoundary } from '../../../components/error-boundary/ErrorBoundary';
import PkgModulesWrapper from '../../../components/module/PkgModulesWrapper';
import { useGetTransaction } from '../../../hooks/useGetTransaction';
import { getOwnerStr } from '../../../utils/objectUtils';
import { trimStdLibPrefix } from '../../../utils/stringUtils';
import { type DataType } from '../ObjectResultType';
import TransactionBlocksForAddress, {
    ObjectFilterValue,
} from '~/components/TransactionBlocksForAddress';
import { AddressLink, ObjectLink } from '~/ui/InternalLink';
import { TabHeader, Tabs, TabsContent, TabsList, TabsTrigger } from '~/ui/Tabs';

import styles from './ObjectView.module.css';

const GENESIS_TX_DIGEST = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=';

const splitPanelsOrientation: { label: string; value: Direction }[] = [
    { label: 'STACKED', value: 'vertical' },
    { label: 'SIDE-BY-SIDE', value: 'horizontal' },
];

function PkgView({ data }: { data: DataType }) {
    const [selectedSplitPanelOrientation, setSplitPanelOrientation] = useState(
        splitPanelsOrientation[1].value,
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
                                    {splitPanelsOrientation.map(({ value, label }) => (
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
