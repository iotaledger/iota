// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { MDXComponents } from 'mdx/types';

export function useMDXComponents(components: MDXComponents): MDXComponents {
    return {
        h1: (props) => <h1 className="text-2xl mb-4" {...props} />,
        p: (props) => <p className="mb-4" {...props} />,
        ul: (props) => <ul className="list-disc list-inside" {...props} />,
        li: (props) => <li className="mb-4" {...props} />,
        ...components,
    };
}
