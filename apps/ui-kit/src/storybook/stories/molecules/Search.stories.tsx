// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import { Search, SearchBarType, Suggestion } from '@/components';

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

        const handleSuggestionsChange = (suggestions: Suggestion[]) => {
            setFilteredSuggestions(suggestions);
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
                    onSuggestionsChange={handleSuggestionsChange}
                    onSuggestionClick={handleSuggestionClick}
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
            { id: '1', label: 'Dashboard' },
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
        onSuggestionsChange: {
            action: 'suggestionsChanged',
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(SearchBarType),
            },
        },
    },
};
