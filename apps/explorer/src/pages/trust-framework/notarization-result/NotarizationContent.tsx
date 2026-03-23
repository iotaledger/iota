// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { AddressAlias, useCopyToClipboard, useGetObjectOrPastObject } from '@iota/core';
import { PageHeader, PageLayout } from '~/components';
import { onCopySuccess } from '~/lib';
import { useNotarizationPkgId } from '~/contexts';
import { useMemo } from 'react';
import { Warning } from '@iota/apps-ui-icons';
import {
    getNotarizationMethod,
    getNotarizationType,
    MetadataBuilder,
} from '../headerMetadataHelper';
import { useResolveNotarization } from '~/hooks/useResolveNotarization';
import { NotarizastionSummaryView } from './views/NotarizationSummaryView';
// import { LockLifecycleView } from './views/LockLifecycleView';
// import { OwnersView } from './views/OwnersView';
// import { SideBySidePanels } from '~/components/ui/SideBySidePanels';
import { TransactionsView } from '../common/TransactionsView';
import { StateView } from './views/StateView';
import { NotarizationJsonView } from './views/NotarizationJsonView';

interface NotarizationContentProps {
    objectId: string;
}

export function NotarizationContent({ objectId }: NotarizationContentProps) {
    const { data: objectResult, isPending: isObjectPending } = useGetObjectOrPastObject(objectId);
    const { data: notarizationDocument, isPending: isNotarizationPending } =
        useResolveNotarization(objectId);

    const copyToClipboard = useCopyToClipboard(onCopySuccess);
    const iotaNotarizationPackage = useNotarizationPkgId();

    const isPending = useMemo(
        () => isNotarizationPending || isObjectPending,
        [isNotarizationPending, isObjectPending],
    );
    if (isPending) {
        return (
            <PageLayout
                loading
                loadingText="Loading Notarization Document and Object..."
                content={[]}
            />
        );
    }

    if (notarizationDocument == null) {
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Error resolving Notarization Document"
                        supportingText={`Could not resolve Notarization ${objectId} in the current network.`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                }
            />
        );
    }

    if (objectResult == null) {
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Error fetching Notarization Object"
                        supportingText={`Could not fetch Object ID ${objectId} from the current network.`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                }
            />
        );
    }

    if (iotaNotarizationPackage == null) {
        // The activation of this branch is a symptom of Notarization WASM Web module not loaded.
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Error loading official Notarization package"
                        supportingText="Could not load package ID from Notarization client."
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                }
            />
        );
    }

    return (
        <PageLayout
            content={
                <div className="flex flex-col gap-y-2xl">
                    <PageHeader
                        type="Notarization"
                        title={
                            <AddressAlias
                                address={objectId || ''}
                                onCopy={() => copyToClipboard(objectId || '')}
                            />
                        }
                        showCopyButton={false}
                        metaItems={MetadataBuilder.create()
                            .addItem(
                                getNotarizationType(
                                    objectResult.data || null,
                                    iotaNotarizationPackage,
                                ),
                            )
                            .addItem(getNotarizationMethod(notarizationDocument))
                            .build()}
                    />
                    <NotarizastionSummaryView
                        objectData={objectResult.data!}
                        notarizationDocument={notarizationDocument}
                    />
                    {/* <SideBySidePanels */}
                    {/*     firstPanel={ */}
                    {/*         <LockLifecycleView */}
                    {/*             locking={notarizationDocument.immutableMetadata.locking} */}
                    {/*         /> */}
                    {/*     } */}
                    {/*     secondPanel={<OwnersView objectId={objectId} />} */}
                    {/* /> */}
                    <StateView notarization={notarizationDocument} />
                    <NotarizationJsonView notarization={notarizationDocument} />
                    <TransactionsView objectId={objectId} />
                </div>
            }
        />
    );
}
