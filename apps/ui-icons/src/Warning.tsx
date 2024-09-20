// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgWarning(props: SVGProps<SVGSVGElement>) {
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
                fillRule="evenodd"
                clipRule="evenodd"
                d="M13.732 3C12.9622 1.66667 11.0377 1.66667 10.2679 3L1.6076 18C0.837803 19.3333 1.80005 21 3.33965 21H20.6602C22.1998 21 23.162 19.3333 22.3922 18L13.732 3ZM12.0003 17.8C12.663 17.8 13.2003 17.2627 13.2003 16.6C13.2003 15.9373 12.663 15.4 12.0003 15.4C11.3376 15.4 10.8003 15.9373 10.8003 16.6C10.8003 17.2627 11.3376 17.8 12.0003 17.8ZM10.8003 8.2C10.8003 7.53726 11.3376 7 12.0003 7C12.663 7 13.2003 7.53726 13.2003 8.2V11.8C13.2003 12.4627 12.663 13 12.0003 13C11.3376 13 10.8003 12.4627 10.8003 11.8V8.2Z"
                fill="currentColor"
            />
            ;
        </svg>
    );
}
