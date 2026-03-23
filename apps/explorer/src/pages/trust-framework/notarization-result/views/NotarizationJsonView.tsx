// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Title, TooltipPosition } from '@iota/apps-ui-kit';
import { type OnChainNotarization } from '@iota/notarization/web';
import { Panel, PanelGroup } from 'react-resizable-panels';
import { ErrorBoundary, SyntaxHighlighter } from '~/components';
import { replaceJsonKeyValue } from '~/lib';

interface NotarizationJsonViewProps {
    notarization: OnChainNotarization;
}

export function NotarizationJsonView({ notarization }: NotarizationJsonViewProps) {
    return (
        <ErrorBoundary>
            <div className="panel-bg flex w-full flex-col rounded-xl border border-transparent p-md--rs">
                <PanelGroup direction="horizontal">
                    <Panel>
                        <div className="flex w-full flex-col gap-sm">
                            <Title
                                title="Notarization"
                                tooltipPosition={TooltipPosition.Left}
                                tooltipText="The raw JSON representation of the On-Chain Notarization. This includes the state, metadata, and other properties of the notarization."
                            />
                            <div className="flex flex-col">
                                <SyntaxHighlighter
                                    code={JSON.stringify(
                                        notarization?.toJSON(),
                                        replaceJsonKeyValue,
                                        2,
                                    )}
                                    language="json"
                                />
                            </div>
                        </div>
                    </Panel>
                </PanelGroup>
            </div>
        </ErrorBoundary>
    );
}
