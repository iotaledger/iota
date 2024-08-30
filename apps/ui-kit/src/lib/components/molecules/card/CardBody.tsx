// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type CardBodyProps = {
    title: string;
    subtitle?: string;
    clickableAction?: React.ReactNode;
    icon?: React.ReactNode;
};

export function CardBody({ title, subtitle, clickableAction, icon }: CardBodyProps) {
    const handleActionCardBodyClick = (event: React.MouseEvent) => {
        event?.stopPropagation();
    };
    return (
        <div className="flex w-full flex-col">
            <div className="flex flex-row items-center gap-x-xxs">
                <div className="font-inter text-title-md text-neutral-10 dark:text-neutral-92">
                    {title}
                </div>
                {icon && <div className="flex items-center">{icon}</div>}
                {clickableAction && (
                    <div onClick={handleActionCardBodyClick} className="flex items-center">
                        {clickableAction}
                    </div>
                )}
            </div>
            {subtitle && (
                <div className="font-inter text-body-md text-neutral-40 dark:text-neutral-60">
                    {subtitle}
                </div>
            )}
        </div>
    );
}
