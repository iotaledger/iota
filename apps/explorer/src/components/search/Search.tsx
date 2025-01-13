// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from 'react';

import { useNavigateWithQuery } from '~/components/ui';
import { ListItem, Search as SearchBox, type Suggestion } from '@iota/apps-ui-kit';
import { useDebouncedValue } from '~/hooks/useDebouncedValue';
import { useSearch } from '~/hooks/useSearch';
import { ampli } from '~/lib/utils';

export function Search(): JSX.Element {
    const [query, setQuery] = useState('');
    const debouncedQuery = useDebouncedValue(query);
    const { isPending, data: results } = useSearch(debouncedQuery);
    const navigate = useNavigateWithQuery();
    const handleSelectResult = useCallback(
        (result: Suggestion) => {
            if (result) {
                ampli.clickedSearchResult({
                    searchQuery: result.id,
                    searchCategory: result.label,
                });
                navigate(`/${result?.type}/${encodeURIComponent(result?.id)}`, {});
                setQuery('');
            }
        },
        [navigate],
    );

    useEffect(() => {
        if (debouncedQuery) {
            ampli.completedSearch({
                searchQuery: debouncedQuery,
            });
        }
    }, [debouncedQuery]);

    return (
        <SearchBox
            searchValue={query}
            onSearchValueChange={(value) => setQuery(value?.trim() ?? '')}
            onSuggestionClick={handleSelectResult}
            placeholder="Search"
            isLoading={isPending || debouncedQuery !== query}
            suggestions={results}
            renderSuggestion={(suggestion) => (
                <div className="flex cursor-pointer justify-between">
                    <ListItem hideBottomBorder>
                        <div className="overflow-hidden text-ellipsis">{suggestion.label}</div>
                        <div className="text-caption text-steel break-words pl-xs font-medium uppercase">
                            {suggestion.type}
                        </div>
                    </ListItem>
                </div>
            )}
        />
    );
}
