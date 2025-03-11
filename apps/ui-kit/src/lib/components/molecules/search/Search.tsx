// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from 'react';
import cx from 'classnames';
import { Loader, Search as SearchIcon } from '@iota/apps-ui-icons';
import { Divider, SearchBarType } from '@/components';
import {
    BACKGROUND_COLORS,
    SUGGESTIONS_WRAPPER_STYLE,
    SEARCH_WRAPPER_STYLE,
} from './search.classes';

export interface Suggestion {
    id: string;
    label: string;
    supportingText?: string;
    type?: string;
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
     * Are the suggestions loading.
     */
    isLoading: boolean;
    /**
     * Callback when a key is pressed.
     */
    onKeyDown?: (event: React.KeyboardEvent<HTMLInputElement>) => void;
    /**
     * The type of the search bar. Can be 'outlined' or 'filled'.
     */
    type?: SearchBarType;
    /**
     * Render suggestion.
     */
    renderSuggestion: (suggestion: Suggestion, index: number) => React.ReactNode;
}

export function Search({
    searchValue,
    suggestions,
    onSearchValueChange,
    onSuggestionClick,
    placeholder,
    isLoading = false,
    onKeyDown,
    type = SearchBarType.Outlined,
    renderSuggestion,
}: SearchProps): React.JSX.Element {
    const inputRef = useRef<HTMLInputElement>(null);
    const suggestionsListRef = useRef<HTMLDivElement>(null);
    const [isSuggestionsVisible, setIsSuggestionsVisible] = useState(true);
    const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

    function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
        const value = event.target.value;
        onSearchValueChange(value);
    }

    // Hide suggestions on escape key press
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                setIsSuggestionsVisible(false);
                inputRef.current?.blur();
            }
        };

        document.addEventListener('keydown', handler);

        return () => {
            document.removeEventListener('keydown', handler);
        };
    }, []);

    // Hide suggestions on click outside
    useEffect(() => {
        const listener = (event: MouseEvent | TouchEvent) => {
            const el = inputRef?.current;
            if (!el || el.contains(event?.target as Node)) {
                return;
            }

            if (suggestionsListRef.current?.contains(event.target as Node)) {
                return;
            }
            setIsSuggestionsVisible(false);
        };

        document.addEventListener('click', listener, true);
        document.addEventListener('touchstart', listener, true);

        return () => {
            document.removeEventListener('click', listener, true);
            document.removeEventListener('touchstart', listener, true);
        };
    }, [inputRef]);

    const showSuggestions = isSuggestionsVisible && suggestions && suggestions.length > 0;

    const roundedStyleWithSuggestions = showSuggestions
        ? 'rounded-t-3xl border-b border-b-transparent'
        : type === SearchBarType.Outlined
          ? 'rounded-3xl border-b'
          : 'rounded-full';
    const searchTypeClass = SEARCH_WRAPPER_STYLE[type];
    const backgroundColorClass = BACKGROUND_COLORS[type];
    const suggestionsStyle = SUGGESTIONS_WRAPPER_STYLE[type];

    const handleOnSuggestionClick = (suggestion: Suggestion) => {
        onSuggestionClick?.(suggestion);
        onSearchValueChange('');
        setIsSuggestionsVisible(false);
        setSelectedIndex(null);
        inputRef.current?.blur();
    };

    const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
        if (suggestions && suggestions?.length > 0) {
            if (event.key === 'ArrowDown') {
                event.preventDefault();
                setSelectedIndex((prev) =>
                    prev === null || prev >= suggestions.length - 1 ? 0 : prev + 1,
                );
            } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setSelectedIndex((prev) =>
                    prev === null || prev <= 0 ? suggestions.length - 1 : prev - 1,
                );
            } else if (event.key === 'Enter') {
                event.preventDefault();
                if (selectedIndex !== null && suggestions[selectedIndex]) {
                    handleOnSuggestionClick(suggestions[selectedIndex]);
                } else if (suggestions.length === 1) {
                    handleOnSuggestionClick(suggestions[0]);
                }
            }
        }
    };

    return (
        <div className="relative w-full">
            <div
                className={cx(
                    'flex items-center overflow-hidden px-md py-sm text-neutral-10 dark:text-neutral-92 [&_svg]:h-6 [&_svg]:w-6',
                    roundedStyleWithSuggestions,
                    searchTypeClass,
                )}
            >
                <input
                    ref={inputRef}
                    type="text"
                    value={searchValue}
                    onChange={handleChange}
                    onKeyDown={handleKeyDown}
                    onFocus={() => setIsSuggestionsVisible(true)}
                    placeholder={placeholder}
                    className={cx(
                        'w-full flex-1 outline-none placeholder:text-neutral-40 placeholder:dark:text-neutral-60',
                        backgroundColorClass,
                    )}
                />
                <SearchIcon />
            </div>
            {showSuggestions && (
                <div
                    ref={suggestionsListRef}
                    className={cx(
                        'absolute left-0 top-full flex w-full flex-col items-center overflow-hidden bg-neutral-100 dark:bg-neutral-10',
                        suggestionsStyle,
                    )}
                >
                    <Divider width="w-11/12" />
                    {isLoading ? (
                        <div className="px-md py-sm">
                            <Loader className="animate-spin" />
                        </div>
                    ) : (
                        suggestions.map((suggestion, index) => (
                            <div
                                key={suggestion.id}
                                onClick={() => handleOnSuggestionClick(suggestion)}
                                onMouseEnter={() => setSelectedIndex(index)}
                                className={cx(
                                    'w-full cursor-pointer px-md py-sm',
                                    selectedIndex === index
                                        ? 'bg-shader-primary-dark-16 dark:bg-shader-inverted-dark-16'
                                        : '',
                                )}
                            >
                                {renderSuggestion(suggestion, index)}
                            </div>
                        ))
                    )}
                </div>
            )}
        </div>
    );
}
