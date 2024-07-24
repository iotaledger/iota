// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import cx from 'classnames';
import { Search as SearchIcon } from '@iota/ui-icons';
import { Divider, ListItem, SearchBarType } from '@/components';
import {
    BACKGROUND_COLORS,
    SUGGESTIONS_WRAPPER_STYLE,
    SEARCH_WRAPPER_STYLE,
} from './search.classes';

export interface SearchProps {
    /**
     * List of suggestions to display (optional).
     */
    suggestions?: string[];
    /**
     * Callback when a suggestion is clicked.
     */
    onSuggestionClick?: (suggestion: string) => void;
    /**
     * Placeholder text for the search input.
     */
    placeholder: string;
    /**
     * Callback when the search input value changes.
     */
    onChange?: (value: string) => void;
    /**
     * The type of the search bar. Can be 'outlined' or 'filled'.
     */
    type?: SearchBarType;
}

export function Search({
    suggestions = [],
    onSuggestionClick,
    placeholder,
    onChange,
    type = SearchBarType.Outlined,
}: SearchProps): React.JSX.Element {
    const [searchValue, setSearchValue] = useState<string>('');
    const [filteredSuggestions, setFilteredSuggestions] = useState<string[]>([]);

    function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
        const value = event.target.value;
        setSearchValue(value);
        onChange && onChange(value);

        if (value) {
            const filtered = suggestions?.filter((suggestion) =>
                suggestion.toLowerCase().includes(value.toLowerCase()),
            );
            setFilteredSuggestions(filtered);
        } else {
            setFilteredSuggestions([]);
        }
    }

    function handleSuggestionClick(suggestion: string) {
        setSearchValue(suggestion);
        setFilteredSuggestions([]);
        onSuggestionClick && onSuggestionClick(suggestion);
    }

    const showSuggestions = filteredSuggestions.length > 0;

    const roundedStyleWithSuggestions = showSuggestions
        ? 'rounded-t-3xl border-b-0'
        : 'rounded-full border-b';
    const searchTypeClass = SEARCH_WRAPPER_STYLE[type];
    const backgroundColorClass = BACKGROUND_COLORS[type];
    const suggestionsStyle = SUGGESTIONS_WRAPPER_STYLE[type];
    return (
        <div className="relative w-full">
            <div
                className={cx(
                    'flex items-center overflow-hidden px-md py-sm',
                    roundedStyleWithSuggestions,
                    searchTypeClass,
                )}
            >
                <input
                    type="text"
                    value={searchValue}
                    onChange={handleChange}
                    placeholder={placeholder}
                    className={cx(
                        'flex-1 text-neutral-10 outline-none placeholder:text-neutral-40 dark:text-neutral-92 placeholder:dark:text-neutral-60',
                        backgroundColorClass,
                    )}
                />
                <SearchIcon className="h-6 w-6 text-neutral-10 dark:text-neutral-92" />
            </div>
            {showSuggestions && (
                <div
                    className={cx(
                        'absolute left-0 top-full flex w-full flex-col items-center overflow-hidden',
                        suggestionsStyle,
                    )}
                >
                    <Divider width="w-11/12" />
                    {filteredSuggestions.map((suggestion, index) => (
                        <ListItem
                            key={index}
                            showRightIcon={false}
                            onClick={() => handleSuggestionClick(suggestion)}
                            hideBottomBorder
                        >
                            {suggestion}
                        </ListItem>
                    ))}
                </div>
            )}
        </div>
    );
}
