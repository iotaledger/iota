// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgSettings(props: SVGProps<SVGSVGElement>) {
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
                fillRule="evenodd"
                d="M11.07 2c-.635 0-1.184.411-1.316.987l-.132.573c-.194.843-.872 1.508-1.677 1.941-.808.435-1.76.64-2.65.374l-.604-.182c-.604-.182-1.264.053-1.581.562l-.93 1.49c-.317.51-.207 1.155.265 1.55l.53.44c.67.559.948 1.4.948 2.234 0 .867-.285 1.743-.982 2.323l-.496.414c-.472.394-.582 1.04-.265 1.549l.93 1.49c.317.51.977.744 1.581.562l.711-.214c.86-.258 1.781-.064 2.564.354.785.42 1.444 1.069 1.633 1.89l.155.676c.132.576.681.987 1.317.987h1.858c.636 0 1.185-.411 1.317-.987l.155-.676c.19-.821.848-1.47 1.633-1.89.782-.418 1.704-.612 2.563-.354l.712.214c.604.182 1.264-.052 1.581-.562l.93-1.49c.317-.51.207-1.155-.265-1.549l-.497-.414c-.696-.58-.982-1.456-.982-2.323 0-.835.28-1.675.95-2.233l.529-.442c.472-.394.582-1.04.265-1.549l-.93-1.49c-.317-.51-.977-.744-1.581-.562l-.604.182c-.89.267-1.843.061-2.65-.374-.805-.433-1.484-1.098-1.678-1.941l-.131-.573C14.113 2.41 13.565 2 12.929 2zm-3.109 9.969C7.961 9.904 9.77 8 12 8s4.038 1.904 4.038 3.969S14.23 16 12 16s-4.039-1.967-4.039-4.031"
                clipRule="evenodd"
            />
        </svg>
    );
}
