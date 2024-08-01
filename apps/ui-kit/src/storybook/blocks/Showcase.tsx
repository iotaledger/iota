// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface ShowcaseProps {
    children?: React.ReactNode;
    title: string;
}

export function Showcase({ children, title }: ShowcaseProps) {
    return (
        <div className="flex flex-col gap-2">
            <code className="inline w-fit rounded-md bg-neutral-96 px-xxs">{title}</code>
            <div className="flex">
                <div className="flex flex-row items-center justify-center rounded-xl border border-neutral-70 p-md">
                    {children}
                </div>
            </div>
        </div>
    );
}
