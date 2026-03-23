// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Title, TooltipPosition } from '@iota/apps-ui-kit';
import { type OnChainNotarization } from '@iota/notarization/web';
import { Panel, PanelGroup } from 'react-resizable-panels';
import { ErrorBoundary, SyntaxHighlighter } from '~/components';
import { useNotarizationState } from '~/hooks/useNotarizationState';

interface StateViewProps {
    notarization: OnChainNotarization;
}

export function StateView({ notarization }: StateViewProps) {
    const { data, isLoading, error } = useNotarizationState(notarization);

    if (isLoading) {
        return <div>Loading...</div>;
    }

    if (error) {
        return <div>Error: {error.message}</div>;
    }

    const { content, lang, metadata } = data!;

    return (
        <ErrorBoundary>
            <div className="panel-bg flex h-full w-full flex-col gap-sm--rs rounded-xl border border-transparent p-md--rs">
                <PanelGroup direction="horizontal">
                    <StatePanel content={content} lang={lang} />
                </PanelGroup>
                <PanelGroup direction="horizontal">
                    <MetadataPanel metadata={metadata} />
                </PanelGroup>
            </div>
        </ErrorBoundary>
    );
}

function StatePanel({ content, lang }: { content: string; lang: string }) {
    return (
        <Panel defaultSize={60} minSize={20}>
            <div className="flex h-full w-full flex-col gap-sm">
                <Title
                    title="Notarization State"
                    tooltipPosition={TooltipPosition.Left}
                    tooltipText="The state data of this Notarization. Displayed as text if valid UTF-8, otherwise as Base64."
                />
                <div className="flex flex-col">
                    <SyntaxHighlighter code={content} language={lang} />
                </div>
            </div>
        </Panel>
    );
}

function MetadataPanel({ metadata }: { metadata?: string }) {
    return (
        <Panel defaultSize={40} minSize={20}>
            <div className="flex h-full w-full flex-col gap-sm">
                <Title
                    title="State Metadata"
                    tooltipPosition={TooltipPosition.Left}
                    tooltipText="The metadata associated with the state."
                />
                <div className="flex flex-col">
                    <SyntaxHighlighter code={metadata!} language="text" />
                </div>
            </div>
        </Panel>
    );
}
