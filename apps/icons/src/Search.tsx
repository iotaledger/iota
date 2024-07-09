// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
const SvgSearch = (props: SVGProps<SVGSVGElement>) => (
    <svg
        xmlns="http://www.w3.org/2000/svg"
        width="1em"
        height="1em"
        fill="none"
        viewBox="0 0 24 24"
        {...props}
    >
        <path
            fill="currentColor"
            fillRule="evenodd"
            d="M12 4a7 7 0 1 0 3.594 13.008l3.699 3.7a1 1 0 0 0 1.414-1.415l-3.555-3.555A7 7 0 0 0 12 4Zm-5 7a5 5 0 1 1 10 0 5 5 0 0 1-10 0Z"
            clipRule="evenodd"
        />
    </svg>
);
export default SvgSearch;
