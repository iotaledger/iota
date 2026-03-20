// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Title } from '@iota/apps-ui-kit';
import { type IotaDocument } from '@iota/identity-wasm/web';
import { Panel, PanelGroup } from 'react-resizable-panels';
import { ErrorBoundary, SyntaxHighlighter } from '~/components';

interface DidDocumentJsonViewProps {
    didDocument: IotaDocument;
}

export function DidDocumentJsonView({ didDocument }: DidDocumentJsonViewProps) {
    return (
        <ErrorBoundary>
            <div className="panel-bg flex w-full flex-col rounded-xl border border-transparent p-md--rs">
                <PanelGroup direction="horizontal">
                    <Panel>
                        <div className="flex w-full flex-col gap-sm">
                            <Title title="DID Document" />
                            <div className="flex flex-col">
                                <SyntaxHighlighter
                                    code={JSON.stringify(didDocument?.toJSON(), null, 2)}
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
