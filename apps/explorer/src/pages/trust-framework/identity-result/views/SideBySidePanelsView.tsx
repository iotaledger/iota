// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Panel, PanelGroup } from 'react-resizable-panels';
import { ErrorBoundary } from '~/components';

interface ControllerAndServiceViewProps {
    firstPanelView: React.ReactNode;
    secondPanelView: React.ReactNode;
}

export function SideBySidePanelsView({
    firstPanelView,
    secondPanelView,
}: ControllerAndServiceViewProps) {
    return (
        <ErrorBoundary>
            <div className="flex flex-col gap-md md:flex-row">
                <div className="flex w-full flex-1 md:w-1/2">
                    <div className="panel-bg flex w-full flex-col rounded-xl border border-transparent p-md--rs">
                        <PanelGroup direction="horizontal">
                            <Panel>
                                <ErrorBoundary>{firstPanelView}</ErrorBoundary>
                            </Panel>
                        </PanelGroup>
                    </div>
                </div>
                <div className="flex w-full md:w-1/2">
                    <div className="panel-bg flex w-full flex-col rounded-xl border border-transparent p-md--rs">
                        <PanelGroup direction="horizontal">
                            <Panel>
                                <ErrorBoundary>{secondPanelView}</ErrorBoundary>
                            </Panel>
                        </PanelGroup>
                    </div>
                </div>
            </div>
        </ErrorBoundary>
    );
}
