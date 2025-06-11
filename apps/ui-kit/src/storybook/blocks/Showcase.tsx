// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface ShowcaseProps {
    children?: React.ReactNode;
    title: string;
}

export function Showcase({ children, title }: ShowcaseProps) {
    return (
        <div className="flex flex-col gap-2">
            <code className="bg-neutral-96 inline w-fit rounded-md px-xxs">{title}</code>
            <div className="flex">
                <div className="border-neutral-70 flex flex-row items-center justify-center rounded-xl border p-md">
                    {children}
                </div>
            </div>
        </div>
    );
}
