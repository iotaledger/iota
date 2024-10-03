// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search24 } from '@iota/icons';
import { type IotaMoveNormalizedStruct, type IotaObjectResponse } from '@iota/iota-sdk/client';
import { Combobox, ComboboxInput, ComboboxList, LoadingIndicator } from '@iota/ui';
import clsx from 'clsx';
import { useCallback, useEffect, useState } from 'react';
import { Banner } from '~/components/ui';
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
} from '@iota/apps-ui-kit';

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
            <Banner variant="error" spacing="lg" fullWidth>
                Failed to get field data for {id}
            </Banner>
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
                                <Combobox value={query} onValueChange={setQuery}>
                                    <div className="flex w-full justify-between rounded-lg border border-white/50 bg-white py-1 pl-3 shadow-dropdownContent">
                                        <ComboboxInput
                                            placeholder="Search"
                                            className="w-full border-none focus:outline-0"
                                        />
                                        <button
                                            className="border-none bg-inherit pr-2"
                                            type="submit"
                                        >
                                            <Search24 className="h-4.5 w-4.5 cursor-pointer fill-steel align-middle text-gray-60" />
                                        </button>
                                    </div>
                                    <ComboboxList
                                        showResultsCount
                                        options={filteredFieldNames.map((item) => ({
                                            value: item.name,
                                            label: item.name,
                                        }))}
                                        onSelect={({ value }) => {
                                            setActiveFieldName(value);
                                        }}
                                    />
                                </Combobox>
                            )}
                            <div
                                className={clsx(
                                    'flex max-h-44 flex-col overflow-y-auto md:max-h-96',
                                    renderSearchBar && 'mt-4',
                                )}
                            >
                                {normalizedStructData?.fields?.map(({ name, type }) => (
                                    <ButtonUnstyled
                                        key={name}
                                        className="rounded-lg p-xs hover:bg-primary-80/20"
                                        onClick={() => onFieldsNameClick(name)}
                                    >
                                        <KeyValueInfo
                                            keyText={name}
                                            value={getFieldTypeValue(type, objectType).displayName}
                                            isTruncated
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
