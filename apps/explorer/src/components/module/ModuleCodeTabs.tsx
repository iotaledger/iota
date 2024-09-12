// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ModuleView from './ModuleView';
import { useVerifiedSourceCode } from '~/hooks/useVerifiedSourceCode';
import {
    ButtonSegment,
    ButtonSegmentType,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';
import { TabbedContentWrapper, ListTabContent } from './TabbedContentWrapper';
import { TabsProvider, TabSelector } from '../tabs';

type ModuleCodeTabsProps = {
    packageId: string;
    moduleName: string;
    moduleBytecode: string;
    isCompact: boolean;
};
interface TabItem {
    id: string;
    label: string;
    hidden?: boolean;
}

export function ModuleCodeTabs({
    packageId,
    moduleName,
    moduleBytecode,
    isCompact,
}: ModuleCodeTabsProps): React.JSX.Element {
    const { data: verifiedSourceCode } = useVerifiedSourceCode({
        packageId,
        moduleName,
    });

    const bytecodeTab: TabItem = {
        id: 'bytecode',
        label: 'Bytecode',
    };

    const sourceTab: TabItem = {
        id: 'source',
        label: 'Source Verified',
        hidden: !verifiedSourceCode,
    };

    const TABS: TabItem[] = [bytecodeTab, sourceTab];

    return (
        <TabbedContentWrapper>
            <TabsProvider tabs={TABS}>
                <TabSelector>
                    {(selectedTabId, setSelectedTabId) => (
                        <SegmentedButton type={SegmentedButtonType.Transparent}>
                            {TABS.filter(({ hidden }) => !hidden).map(({ id, label }) => (
                                <ButtonSegment
                                    key={id}
                                    type={ButtonSegmentType.Underlined}
                                    onClick={() => setSelectedTabId(id)}
                                    label={label}
                                    selected={selectedTabId === id}
                                />
                            ))}
                        </SegmentedButton>
                    )}
                </TabSelector>

                <ListTabContent id={bytecodeTab.id} isCompact={isCompact}>
                    <ModuleView id={packageId} name={moduleName} code={moduleBytecode} />
                </ListTabContent>

                {verifiedSourceCode && (
                    <ListTabContent id={sourceTab.id} isCompact={isCompact}>
                        <ModuleView id={packageId} name={moduleName} code={verifiedSourceCode} />
                    </ListTabContent>
                )}
            </TabsProvider>
        </TabbedContentWrapper>
    );
}
