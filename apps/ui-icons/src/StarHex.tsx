// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgStarHex(props: SVGProps<SVGSVGElement>) {
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
                d="M12.182 6.399a.2.2 0 0 0-.364 0l-1.453 3.183a.2.2 0 0 1-.16.116l-3.476.398a.2.2 0 0 0-.113.346l2.579 2.366a.2.2 0 0 1 .06.187l-.695 3.43a.2.2 0 0 0 .295.214l3.047-1.721a.2.2 0 0 1 .196 0l3.047 1.72a.2.2 0 0 0 .295-.213l-.696-3.43a.2.2 0 0 1 .061-.187l2.579-2.366a.2.2 0 0 0-.113-.346l-3.477-.398a.2.2 0 0 1-.159-.116z"
            />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M13.457 1.666a3 3 0 0 0-2.914 0l-7 3.889A3 3 0 0 0 2 8.177v7.646a3 3 0 0 0 1.543 2.623l7 3.889a3 3 0 0 0 2.914 0l7-3.89A3 3 0 0 0 22 15.824V8.177a3 3 0 0 0-1.543-2.622l-7-3.89Zm-1.943 1.748a1 1 0 0 1 .972 0l7 3.889a1 1 0 0 1 .514.874v7.646a1 1 0 0 1-.514.875l-7 3.888a1 1 0 0 1-.972 0l-7-3.888A1 1 0 0 1 4 15.823V8.177a1 1 0 0 1 .514-.874l7-3.89Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
