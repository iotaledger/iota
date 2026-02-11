// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { AddressAlias, useCopyToClipboard, useGetObjectOrPastObject } from '@iota/core';
import type { IotaDID } from '@iota/identity-wasm/web';
import { PageHeader, PageLayout } from '~/components';
import { useResolveDid } from '~/hooks/useResolveDid';
import { onCopySuccess } from '~/lib';
import { useIdentityPkgId } from '~/contexts';
import { useMemo } from 'react';
import { Warning } from '@iota/apps-ui-icons';
import { getIdentityType, getLegacyMetadata, MetadataBuilder } from '../headerMetadataHelper';
import { ControllerView } from './ControllerView';
import { ServiceView } from './ServiceView';
import { DidSummaryView } from './DidSummaryView';
import { DidDocumentJsonView } from './DidDocumentJsonView';
import { SideBySidePanelsView } from './SideBySidePanelsView';
import { TransactionsView } from './TransactionsView';

interface DidContentProps {
    did: IotaDID;
}

export function DidContent({ did }: DidContentProps) {
    const { didDocument, isPending: isDidDocumentPending } = useResolveDid(did);
    const { data: objectResult, isPending: isObjectPending } = useGetObjectOrPastObject(did.tag());
    const didObject = useMemo(() => objectResult?.data || null, [objectResult?.data]);

    const copyToClipboard = useCopyToClipboard(onCopySuccess);
    const iotaIdentityPackage = useIdentityPkgId();

    const isPending = useMemo(
        () => isDidDocumentPending || isObjectPending,
        [isDidDocumentPending, isObjectPending],
    );
    if (isPending) {
        return <PageLayout loading loadingText="Loading DID Document and Object..." content={[]} />;
    }

    if (didDocument == null) {
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Error resolving DID Document"
                        supportingText={`Could not resolve the DID ${did.toString()} in the current network.`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                }
            />
        );
    }

    if (didObject == null) {
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Error fetching DID Object"
                        supportingText={`Could not fetch DID Object ${did.tag()} from the current network.`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                }
            />
        );
    }

    if (iotaIdentityPackage == null) {
        // The activation of this branch is a symptom of Identity WASM Web module not loaded.
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Error loading official Identity package"
                        supportingText="Could not load package ID from Identity client."
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
                        type="DID"
                        title={
                            <AddressAlias
                                address={did.toString() || ''}
                                onCopy={() => copyToClipboard(did.toString() || '')}
                            />
                        }
                        showCopyButton={false}
                        metaItems={MetadataBuilder.create()
                            .addItem(getIdentityType(didObject, iotaIdentityPackage))
                            .addItem(getLegacyMetadata(didObject))
                            .build()}
                    />
                    <DidSummaryView objectData={didObject} didDocument={didDocument} />
                    <SideBySidePanelsView
                        firstPanelView={<ControllerView objectData={didObject} />}
                        secondPanelView={<ServiceView didDocument={didDocument} />}
                    />
                    <DidDocumentJsonView didDocument={didDocument} />
                    <TransactionsView objectId={did.tag()} />
                </div>
            }
        />
    );
}
