// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageType,
} from '@iota/apps-ui-kit';

export interface ItemProps {
    icon: React.ReactNode;
    title: string;
    subtitle?: string;
    onClick?: () => void;
    isDisabled?: boolean;
}

function MenuListItem({ icon, title, subtitle, onClick, isDisabled }: ItemProps) {
    return (
        <Card type={CardType.Default} onClick={onClick} isDisabled={isDisabled}>
            <CardImage type={ImageType.BgSolid}>
                <div className="flex h-10 w-10 items-center justify-center rounded-full  text-neutral-10 [&_svg]:h-5 [&_svg]:w-5">
                    <span className="text-2xl">{icon}</span>
                </div>
            </CardImage>
            <CardBody title={title} subtitle={subtitle} />
            <CardAction type={CardActionType.Link} />
        </Card>
    );
    // const Component = to ? Link : 'div';

    // const MenuItemContent = (
    //     <>
    //         <div className="flex flex-1 basis-3/5 flex-nowrap items-center gap-2 overflow-hidden">
    //             <div className="text-steel flex flex-none text-2xl">{icon}</div>
    //             <div className="text-gray-90 flex flex-1 text-body font-semibold">{title}</div>
    //         </div>
    //         {subtitle || iconAfter || to ? (
    //             <div
    //                 className={clsx(
    //                     { 'flex-1 basis-2/5': Boolean(subtitle) },
    //                     'flex flex-nowrap items-center justify-end gap-1 overflow-hidden',
    //                 )}
    //             >
    //                 {subtitle ? (
    //                     <div className="text-steel-dark group-hover:text-steel-darker text-bodySmall font-medium transition">
    //                         {subtitle}
    //                     </div>
    //                 ) : null}
    //                 <div className="text-steel group-hover:text-steel-darker flex flex-none text-base transition">
    //                     {iconAfter || (to && <ChevronRight16 />) || null}
    //                 </div>
    //             </div>
    //         ) : null}
    //     </>
    // );

    // if (href) {
    //     return (
    //         <a
    //             href={href}
    //             target="_blank"
    //             rel="noreferrer noopener"
    //             className="group flex cursor-pointer flex-nowrap items-center gap-5 overflow-hidden px-1 py-4.5 no-underline first:pb-3 first:pt-3 last:pb-3"
    //         >
    //             {MenuItemContent}
    //         </a>
    //     );
    // }
    // return (
    //     <Component
    //         data-testid={title}
    //         className="group flex cursor-pointer flex-nowrap items-center gap-5 overflow-hidden px-1 py-5 no-underline first:pb-3 first:pt-3 last:pb-3"
    //         to={to}
    //         onClick={onClick}
    //     >
    //         {MenuItemContent}
    //     </Component>
    // );
}

export default MenuListItem;
