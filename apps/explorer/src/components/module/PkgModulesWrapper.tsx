// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from 'react';
import { type Direction } from 'react-resizable-panels';

import { SplitPanes, useSearchParamsMerged, VerticalList } from '~/components/ui';
import {
    ButtonSegment,
    ButtonSegmentType,
    Divider,
    ListItem,
    Search,
    SegmentedButton,
    SegmentedButtonType,
    type Suggestion,
} from '@iota/apps-ui-kit';
import { ModuleFunctionsInteraction } from './module-functions-interaction';
import { ModuleCodeTabs } from './ModuleCodeTabs';
import { TabbedContentWrapper, ListTabContent } from './TabbedContentWrapper';
import { TabsProvider, type TabItem } from '../tabs';

type ModuleType = [moduleName: string, code: string];

interface PkgModulesWrapperProps {
    id: string;
    modules: ModuleType[];
    splitPanelOrientation: Direction;
}

export function PkgModulesWrapper({
    id,
    modules,
    splitPanelOrientation,
}: PkgModulesWrapperProps): JSX.Element {
    const [searchParams, setSearchParams] = useSearchParamsMerged();
    const [query, setQuery] = useState('');

    const moduleNameValue = searchParams.get('module');
    const moduleFromParams = moduleNameValue
        ? modules.find(([moduleName]) => moduleName === moduleNameValue)
        : undefined;

    // Extract module in URL or default to first module in the list
    const [selectedModuleName, selectedModuleCode] = moduleFromParams ?? modules[0];

    // If module in URL exists but is not in module list, then delete module from URL
    useEffect(() => {
        if (!moduleFromParams) {
            setSearchParams({}, { replace: true });
        }
    }, [setSearchParams, moduleFromParams]);

    const moduleNames = modules.map(([name]) => name);
    const filteredModules = query
        ? moduleNames.filter((name) => name.toLowerCase().includes(query.toLowerCase()))
        : [];

    const submitSearch = useCallback(() => {
        if (filteredModules.length === 1) {
            setSearchParams({
                module: filteredModules[0],
            });
        }
    }, [filteredModules, setSearchParams]);

    const onChangeModule = (newModule: string) => {
        setSearchParams(
            {
                module: newModule,
            },
            {
                preventScrollReset: true,
            },
        );
    };

    const panelContent = [
        {
            panel: (
                <ModuleCodeTabs
                    packageId={id}
                    moduleName={selectedModuleName}
                    moduleBytecode={selectedModuleCode}
                />
            ),
            defaultSize: 40,
        },
        {
            panel: <ExecutePanelContent packageId={id} moduleName={selectedModuleName} />,
            defaultSize: 60,
        },
    ];

    const searchSuggestions: Suggestion[] = filteredModules.map((item) => ({
        id: item,
        label: item,
    }));

    function handleSearchKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
        if (e.key === 'Enter') {
            submitSearch();
        }
    }
    return (
        <div className="flex h-full flex-col items-stretch gap-md--rs md:flex-row md:flex-nowrap">
            <div className="flex w-full flex-col md:min-h-[560px] md:w-1/5">
                <div className="relative z-[1]">
                    <Search
                        onKeyDown={handleSearchKeyDown}
                        searchValue={query}
                        onSearchValueChange={(value) => setQuery(value?.trim() ?? '')}
                        placeholder="Search"
                        isLoading={false}
                        suggestions={searchSuggestions}
                        renderSuggestion={(suggestion) => (
                            <div className="z-10 flex cursor-pointer justify-between">
                                <ListItem
                                    hideBottomBorder
                                    onClick={() => onChangeModule(suggestion.label)}
                                >
                                    <div className="overflow-hidden text-ellipsis">
                                        {suggestion.label}
                                    </div>
                                    <div className="text-caption text-steel break-words pl-xs font-medium uppercase">
                                        {suggestion.type}
                                    </div>
                                </ListItem>
                            </div>
                        )}
                    />
                </div>
                <div className="max-h-[560px] flex-1 overflow-auto pt-sm">
                    <VerticalList>
                        <div className="flex flex-col gap-sm">
                            {moduleNames.map((name) => (
                                <ButtonSegment
                                    key={name}
                                    type={ButtonSegmentType.Underlined}
                                    selected={name === selectedModuleName}
                                    onClick={() => onChangeModule(name)}
                                    label={name}
                                />
                            ))}
                        </div>
                    </VerticalList>
                </div>
            </div>
            <div className="block pt-sm md:hidden">
                <Divider />
            </div>
            <div className="hidden w-4/5 md:block">
                <SplitPanes direction={splitPanelOrientation} splitPanels={panelContent} />
            </div>
            <div className="block md:hidden">
                {panelContent.map((panel, index) => (
                    <div key={index}>{panel.panel}</div>
                ))}
            </div>
        </div>
    );
}

function ExecutePanelContent({
    packageId,
    moduleName,
}: {
    packageId: string;
    moduleName: string;
}): React.JSX.Element {
    const EXECUTE_TAB: TabItem = {
        id: 'execute',
        label: 'Execute',
    };
    const TABS: TabItem[] = [EXECUTE_TAB];

    return (
        <TabbedContentWrapper>
            <TabsProvider tabs={TABS}>
                <SegmentedButton type={SegmentedButtonType.Transparent}>
                    {TABS.map(({ id, label }) => (
                        <ButtonSegment
                            type={ButtonSegmentType.Underlined}
                            label={label}
                            key={id}
                            selected
                        />
                    ))}
                </SegmentedButton>

                <div className="max-h-[560px] overflow-y-auto pr-md--rs">
                    <ListTabContent id={EXECUTE_TAB.id}>
                        <ModuleFunctionsInteraction
                            key={`${packageId}-${moduleName}`}
                            packageId={packageId}
                            moduleName={moduleName}
                        />
                    </ListTabContent>
                </div>
            </TabsProvider>
        </TabbedContentWrapper>
    );
}
