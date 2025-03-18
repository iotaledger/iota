// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgVisibilityOn(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path fill="currentColor" d="M12 14a2 2 0 1 0 0-4 2 2 0 0 0 0 4" />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M19.983 9.805c1.29 1.212 1.29 3.178 0 4.39C18.157 15.908 15.26 18 12 18c-3.261 0-6.157-2.092-7.983-3.805-1.29-1.212-1.29-3.178 0-4.39C5.843 8.092 8.74 6 12 6s6.157 2.092 7.983 3.805M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0"
                clipRule="evenodd"
            />
        </svg>
    );
}
