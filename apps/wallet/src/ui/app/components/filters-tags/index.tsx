// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cl from 'clsx';
import { memo, useEffect, useState } from 'react';
import ReactDOM from 'react-dom';
import { NavLink } from 'react-router-dom';

import st from './Filters.module.scss';

const ELEMENT_ID = '#iota-apps-filters';

// TODO: extend this interface to include params and functions for the filter tags
export interface Tag {
    name: string;
    link: string;
}

interface FiltersPortalProps {
    tags: Tag[];
    firstLastMargin?: boolean;
    callback?: (tag: Tag) => void;
}

function FiltersPortalComponent({ tags, callback, firstLastMargin }: FiltersPortalProps) {
    const [element, setElement] = useState<HTMLElement | null>(null);

    useEffect(() => {
        const content = document.querySelector(ELEMENT_ID) as HTMLElement;
        if (content) setElement(content);
    }, []);

    return (
        <>
            {element
                ? ReactDOM.createPortal(
                      <div className={st.filterTags}>
                          {tags.map((tag) => {
                              return (
                                  <NavLink
                                      id={tag.link}
                                      key={tag.link}
                                      to={`/${tag.link}`}
                                      end
                                      className={({ isActive }) => {
                                          return cl(
                                              { [st.active]: isActive },
                                              st.filter,
                                              firstLastMargin && 'first:ml-3 last:mr-3',
                                          );
                                      }}
                                      title={tag.name}
                                      onClick={callback ? () => callback(tag) : undefined}
                                  >
                                      <span className={st.title}>{tag.name}</span>
                                  </NavLink>
                              );
                          })}
                      </div>,
                      element,
                  )
                : null}
        </>
    );
}

export const FiltersPortal = memo(FiltersPortalComponent);
