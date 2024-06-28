// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from 'react';

import { Search as SearchBox, useNavigateWithQuery, type SearchResult } from '~/components/ui';
import { useDebouncedValue } from '~/hooks/useDebouncedValue';
import { useSearch } from '~/hooks/useSearch';
import { ampli } from '~/lib/utils';

function Search(): JSX.Element {
    const [query, setQuery] = useState('');
    const debouncedQuery = useDebouncedValue(query);
    const { isPending, data: results } = useSearch(debouncedQuery);
    const navigate = useNavigateWithQuery();
    const handleSelectResult = useCallback(
        (result: SearchResult) => {
            if (result) {
                ampli.clickedSearchResult({
                    searchQuery: result.id,
                    searchCategory: result.type,
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
        <div className="max-w flex">
            <SearchBox
                queryValue={query}
                onChange={(value) => setQuery(value?.trim() ?? '')}
                onSelectResult={handleSelectResult}
                placeholder="Search"
                isLoading={isPending || debouncedQuery !== query}
                options={results}
            />
        </div>
    );
}

export default Search;
