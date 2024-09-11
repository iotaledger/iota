// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search24 } from '@iota/icons';
import { type IotaMoveNormalizedStruct, type IotaObjectResponse } from '@iota/iota-sdk/client';
import { Combobox, ComboboxInput, ComboboxList, LoadingIndicator, Text } from '@iota/ui';
import clsx from 'clsx';
import { useCallback, useEffect, useState } from 'react';

import { FieldCollapsible, FieldsCard, FieldsContainer } from '~/components';
import { Banner, Description } from '~/components/ui';
import { getFieldTypeValue } from '~/lib/ui';
import { FieldItem } from './FieldItem';
import { ScrollToViewCard } from './ScrollToViewCard';

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
        <FieldsContainer>
            <div className="w-full md:w-1/5">
                {renderSearchBar && (
                    <Combobox value={query} onValueChange={setQuery}>
                        <div className="flex w-full justify-between rounded-lg border border-white/50 bg-white py-1 pl-3 shadow-dropdownContent">
                            <ComboboxInput
                                placeholder="Search"
                                className="w-full border-none focus:outline-0"
                            />
                            <button className="border-none bg-inherit pr-2" type="submit">
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
                        'flex max-h-44 flex-col overflow-y-auto pr-2 md:max-h-96',
                        renderSearchBar && 'mt-4',
                    )}
                >
                    {normalizedStructData?.fields?.map(({ name, type }) => (
                        <button
                            type="button"
                            key={name}
                            className="mt-0.5 rounded-lg border border-transparent p-2.5 hover:border-iota-primaryBlue2023/20 hover:bg-white/60"
                            onClick={() => onFieldsNameClick(name)}
                        >
                            <Description
                                title={name}
                                titleVariant="body/medium"
                                titleColor="steel-darker"
                                alignItems="center"
                            >
                                <Text uppercase variant="subtitle/normal" color="steel" truncate>
                                    {getFieldTypeValue(type, objectType).displayName}
                                </Text>
                            </Description>
                        </button>
                    ))}
                </div>
            </div>

            <FieldsCard>
                {normalizedStructData?.fields.map(({ name, type }, index) => (
                    <ScrollToViewCard key={name} inView={name === activeFieldName}>
                        <FieldCollapsible
                            open={openFieldsName[name]}
                            onOpenChange={onSetOpenFieldsName(name)}
                            name={name}
                            noMarginBottom={index === normalizedStructData?.fields.length - 1}
                        >
                            <FieldItem
                                value={fieldsData[name]}
                                objectType={objectType}
                                type={type}
                            />
                        </FieldCollapsible>
                    </ScrollToViewCard>
                ))}
            </FieldsCard>
        </FieldsContainer>
    );
}
