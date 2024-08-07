// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Search as SearchIcon } from '@iota/ui-icons';
import { Divider, ListItem, SearchBarType } from '@/components';
import {
    BACKGROUND_COLORS,
    SUGGESTIONS_WRAPPER_STYLE,
    SEARCH_WRAPPER_STYLE,
} from './search.classes';

export interface Suggestion {
    id: string;
    label: string;
}

export interface SearchProps {
    /**
     * The value of the search input.
     */
    searchValue: string;
    /**
     * Callback when the search input value changes.
     */
    onSearchValueChange: (value: string) => void;
    /**
     * List of suggestions to display (optional).
     */
    suggestions?: Suggestion[];
    /**
     * Callback when a suggestion is clicked.
     */
    onSuggestionClick?: (suggestion: Suggestion) => void;
    /**
     * Placeholder text for the search input.
     */
    placeholder: string;
    /**
     * Callback when a key is pressed.
     */
    onKeyDown?: (event: React.KeyboardEvent<HTMLInputElement>) => void;
    /**
     * The type of the search bar. Can be 'outlined' or 'filled'.
     */
    type?: SearchBarType;
}

export function Search({
    searchValue,
    suggestions,
    onSearchValueChange,
    placeholder,
    onSuggestionClick,
    onKeyDown,
    type = SearchBarType.Outlined,
}: SearchProps): React.JSX.Element {
    function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
        const value = event.target.value;
        onSearchValueChange(value);
    }

    function handleSuggestionClick(suggestion: Suggestion) {
        onSearchValueChange(suggestion.label);
        onSuggestionClick?.(suggestion);
    }

    const showSuggestions = suggestions && suggestions.length > 0;

    const roundedStyleWithSuggestions = showSuggestions
        ? 'rounded-t-3xl border-b-0'
        : type === SearchBarType.Outlined
          ? 'rounded-3xl border-b'
          : 'rounded-full';
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
                    onKeyDown={onKeyDown}
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
                    {suggestions.map((suggestion) => (
                        <ListItem
                            key={suggestion.id}
                            showRightIcon={false}
                            onClick={() => handleSuggestionClick(suggestion)}
                            hideBottomBorder
                        >
                            {suggestion.label}
                        </ListItem>
                    ))}
                </div>
            )}
        </div>
    );
}
