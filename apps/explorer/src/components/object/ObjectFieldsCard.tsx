// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaMoveNormalizedStruct, type IotaObjectResponse } from '@iota/iota-sdk/client';
import clsx from 'clsx';
import { useCallback, useEffect, useState } from 'react';
import { getFieldTypeValue } from '~/lib/ui';
import { FieldItem } from './FieldItem';
import { ScrollToViewCard } from './ScrollToViewCard';
import {
    Accordion,
    AccordionHeader,
    Title,
    AccordionContent,
    ButtonUnstyled,
    KeyValueInfo,
    Panel,
    TitleSize,
    LoadingIndicator,
    Search,
    ListItem,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

const DEFAULT_OPEN_FIELDS = 3;
const DEFAULT_FIELDS_COUNT_TO_SHOW_SEARCH = 10;

interface ObjectFieldsProps {
    id: string;
    normalizedStructData?: IotaMoveNormalizedStruct;
    iotaObjectResponseData?: IotaObjectResponse;
    loading: boolean;
    error: boolean;
    objectType?: string;
}

export function ObjectFieldsCard({
    id,
    normalizedStructData,
    iotaObjectResponseData,
    loading,
    error,
    objectType,
}: ObjectFieldsProps): JSX.Element | null {
    const [query, setQuery] = useState('');
    const [activeFieldName, setActiveFieldName] = useState('');
    const [openFieldsName, setOpenFieldsName] = useState<{
        [name: string]: boolean;
    }>({});

    useEffect(() => {
        if (normalizedStructData?.fields) {
            setOpenFieldsName(
                normalizedStructData.fields.reduce(
                    (acc, { name }, index) => {
                        acc[name] = index < DEFAULT_OPEN_FIELDS;
                        return acc;
                    },
                    {} as { [name: string]: boolean },
                ),
            );
        }
    }, [normalizedStructData?.fields]);

    const onSetOpenFieldsName = useCallback(
        (name: string) => (open: boolean) => {
            setOpenFieldsName((prev) => ({
                ...prev,
                [name]: open,
            }));
        },
        [],
    );

    const onFieldsNameClick = useCallback(
        (name: string) => {
            setActiveFieldName(name);
            onSetOpenFieldsName(name)(true);
        },
        [onSetOpenFieldsName],
    );

    if (loading) {
        return (
            <div className="flex w-full justify-center">
                <LoadingIndicator text="Loading data" />
            </div>
        );
    }
    if (error) {
        return (
            <InfoBox
                title="Error loading data"
                supportingText={`Failed to get field data for ${id}`}
                icon={<Warning />}
                type={InfoBoxType.Error}
                style={InfoBoxStyle.Elevated}
            />
        );
    }

    const fieldsData =
        iotaObjectResponseData?.data?.content?.dataType === 'moveObject'
            ? (iotaObjectResponseData?.data?.content?.fields as Record<
                  string,
                  string | number | object
              >)
            : null;

    // Return null if there are no fields
    if (!fieldsData || !normalizedStructData?.fields || !objectType) {
        return null;
    }

    const filteredFieldNames =
        query === ''
            ? normalizedStructData?.fields
            : normalizedStructData?.fields.filter(({ name }) =>
                  name.toLowerCase().includes(query.toLowerCase()),
              );

    const renderSearchBar =
        normalizedStructData?.fields.length >= DEFAULT_FIELDS_COUNT_TO_SHOW_SEARCH;

    return (
        <div className="flex flex-col gap-md md:flex-row">
            <div className="flex w-full flex-1 md:w-1/3">
                <div className="w-full">
                    <Panel hasBorder>
                        <div className="flex flex-col gap-md p-xs">
                            {renderSearchBar && (
                                <Search
                                    searchValue={query}
                                    onSearchValueChange={setQuery}
                                    placeholder="Search"
                                    suggestions={
                                        query.length > 0
                                            ? filteredFieldNames.map((item) => ({
                                                  id: item.name,
                                                  type: item.name,
                                                  label: item.name,
                                              }))
                                            : []
                                    }
                                    onSuggestionClick={(suggestion) => {
                                        setActiveFieldName(suggestion.id);
                                    }}
                                    isLoading={false}
                                    renderSuggestion={(suggestion) => (
                                        <div className="flex cursor-pointer justify-between">
                                            <ListItem hideBottomBorder>
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
                            )}
                            <div
                                className={clsx(
                                    'flex max-h-44 flex-col overflow-y-auto md:max-h-96',
                                    renderSearchBar && 'mt-4',
                                )}
                            >
                                {filteredFieldNames?.map(({ name, type }) => (
                                    <ButtonUnstyled
                                        key={name}
                                        className="rounded-lg p-xs hover:bg-primary-80/20"
                                        onClick={() => onFieldsNameClick(name)}
                                    >
                                        <KeyValueInfo
                                            keyText={name}
                                            value={getFieldTypeValue(type, objectType).displayName}
                                            isTruncated
                                            fullwidth
                                        />
                                    </ButtonUnstyled>
                                ))}
                            </div>
                        </div>
                    </Panel>
                </div>
            </div>
            <div className="flex w-full md:w-2/3">
                <Panel hasBorder>
                    <div className="flex flex-col gap-md p-md--rs">
                        {normalizedStructData?.fields.map(({ name, type }, index) => (
                            <ScrollToViewCard key={name} inView={name === activeFieldName}>
                                <Accordion>
                                    <AccordionHeader
                                        isExpanded={openFieldsName[name]}
                                        onToggle={() =>
                                            onSetOpenFieldsName(name)(!openFieldsName[name])
                                        }
                                    >
                                        <Title size={TitleSize.Small} title={name ?? ''} />
                                    </AccordionHeader>
                                    <AccordionContent isExpanded={openFieldsName[name]}>
                                        <div className="p-md--rs">
                                            <FieldItem
                                                value={fieldsData[name]}
                                                objectType={objectType}
                                                type={type}
                                            />
                                        </div>
                                    </AccordionContent>
                                </Accordion>
                            </ScrollToViewCard>
                        ))}
                    </div>
                </Panel>
            </div>
        </div>
    );
}
