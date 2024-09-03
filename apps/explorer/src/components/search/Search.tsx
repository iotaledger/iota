// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from 'react';

import { useNavigateWithQuery } from '~/components/ui';
import { ListItem, Search as SearchBox, type Suggestion } from '@iota/apps-ui-kit';
import { useDebouncedValue } from '~/hooks/useDebouncedValue';
import { useSearch } from '~/hooks/useSearch';
import { ampli } from '~/lib/utils';

function Search(): JSX.Element {
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
        <div className="flex w-8/12 text-body-md">
            <SearchBox
                searchValue={query}
                onSearchValueChange={(value) => setQuery(value?.trim() ?? '')}
                onSuggestionClick={handleSelectResult}
                placeholder="Search"
                isLoading={isPending || debouncedQuery !== query}
                suggestions={results}
                renderSuggestion={(suggestion) => (
                    <div className="flex cursor-pointer justify-between bg-neutral-98">
                        <ListItem hideBottomBorder>
                            <div className="overflow-hidden text-ellipsis">{suggestion.label}</div>
                            <div className="break-words pl-xs text-caption font-medium uppercase text-steel">
                                {suggestion.type}
                            </div>
                        </ListItem>
                    </div>
                )}
            />
        </div>
    );
}

export default Search;
