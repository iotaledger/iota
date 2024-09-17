// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgImportPass(props: SVGProps<SVGSVGElement>) {
    return (
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
                d="M12 2h1v9l2-2 1.414 1.414-3.707 3.293a1.002 1.002 0 0 1-1.414 0l-3.707-3.293L9 9l2 2V2h1Z"
            />
            <path
                fill="currentColor"
                d="M8 4c-4.275 0-7 3.175-7 7 0 3.406 2.627 6.141 5.975 6.829.471.097.692.434.692.671v2.043c0 1.428 1.807 2.048 2.684.92L13.045 18h2.288C19.466 18 23 14.963 23 11c0-3.825-2.725-7-7-7a1 1 0 1 0 0 2c3.09 0 5 2.197 5 5 0 2.665-2.435 5-5.667 5h-3.267l-2.4 3.085V18.5c0-.729-.219-1.546-.666-2-.424-.43-1.062-.515-1.622-.63C4.797 15.34 3 13.3 3 11c0-2.803 1.91-5 5-5a1 1 0 0 0 0-2Z"
            />
        </svg>
    );
}
