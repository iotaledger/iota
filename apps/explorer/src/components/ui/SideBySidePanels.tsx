// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Panel, PanelGroup } from 'react-resizable-panels';
import { ErrorBoundary } from '~/components';

/**
 * The props for the SideBySidePanels component.
 */
interface SideBySidePanelsProps {
    /**
     * The component for the first panel.
     */
    firstPanel: React.ReactNode;
    /**
     * The component for the second panel.
     */
    secondPanel: React.ReactNode;
}

/**
 * Component that displays two panels side by side, already wrapped in ErrorBoundary.
 */
export function SideBySidePanels({ firstPanel, secondPanel }: SideBySidePanelsProps) {
    return (
        <ErrorBoundary>
            <div className="flex flex-col gap-md md:flex-row">
                <div className="flex w-full flex-1 md:w-1/2">
                    <div className="panel-bg flex w-full flex-col rounded-xl border border-transparent p-md--rs">
                        <PanelGroup direction="horizontal">
                            <Panel>
                                <ErrorBoundary>{firstPanel}</ErrorBoundary>
                            </Panel>
                        </PanelGroup>
                    </div>
                </div>
                <div className="flex w-full md:w-1/2">
                    <div className="panel-bg flex w-full flex-col rounded-xl border border-transparent p-md--rs">
                        <PanelGroup direction="horizontal">
                            <Panel>
                                <ErrorBoundary>{secondPanel}</ErrorBoundary>
                            </Panel>
                        </PanelGroup>
                    </div>
                </div>
            </div>
        </ErrorBoundary>
    );
}
