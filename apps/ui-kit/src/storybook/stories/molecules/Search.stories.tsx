// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import type { Suggestion } from '@/components';
import { ListItem, Search, SearchBarType } from '@/components';
import cx from 'classnames';

const meta: Meta<typeof Search> = {
    component: Search,
    tags: ['autodocs'],
    render: (props) => {
        const [searchValue, setSearchValue] = useState<string>('');
        const [filteredSuggestions, setFilteredSuggestions] = useState<Suggestion[]>([]);

        const handleSearchValueChange = (value: string) => {
            setSearchValue(value);
            const filtered =
                props.suggestions?.filter((suggestion) =>
                    suggestion.label.toLowerCase().includes(value.toLowerCase()),
                ) || [];
            setFilteredSuggestions(filtered);
        };

        const handleSuggestionClick = (suggestion: Suggestion) => {
            setSearchValue(suggestion.label);
            setFilteredSuggestions([]);
        };

        return (
            <div className="h-60">
                <Search
                    {...props}
                    searchValue={searchValue}
                    suggestions={filteredSuggestions}
                    onSearchValueChange={handleSearchValueChange}
                    onSuggestionClick={handleSuggestionClick}
                    renderSuggestion={(suggestion) => (
                        <ListItem
                            key={suggestion.id}
                            showRightIcon={false}
                            onClick={() => handleSuggestionClick(suggestion)}
                            hideBottomBorder
                        >
                            <div
                                className={cx(
                                    'flex w-full flex-row items-center gap-xs',
                                    suggestion.supportingText ? 'justify-between' : 'justify-start',
                                )}
                            >
                                <span className="text-body-lg text-neutral-10 dark:text-neutral-92">
                                    {suggestion.label}
                                </span>
                                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                                    {suggestion.supportingText}
                                </span>
                            </div>
                        </ListItem>
                    )}
                />
            </div>
        );
    },
};

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        suggestions: [
            { id: '1', label: 'Dashboard', supportingText: 'Caption' },
            { id: '2', label: 'Wallet' },
            { id: '3', label: 'Explorer' },
            { id: '4', label: 'SDK' },
        ],
        placeholder: 'Search for tooling apps',
    },
    argTypes: {
        suggestions: {
            control: 'object',
        },
        onSuggestionClick: {
            action: 'suggestionClicked',
        },
        placeholder: {
            control: 'text',
        },
        onSearchValueChange: {
            action: 'searchValueChanged',
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(SearchBarType),
            },
        },
    },
};
