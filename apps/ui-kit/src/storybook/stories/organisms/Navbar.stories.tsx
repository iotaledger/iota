// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Activity, Apps, Assets, Home } from '@iota/apps-ui-icons';
import { Navbar, NavbarSlideout, NavbarItemWithId, NavbarProps } from '@/components';
import { useState } from 'react';

const NAVBAR_ITEMS: NavbarItemWithId[] = [
    { id: 'home', icon: <Home /> },
    { id: 'assets', icon: <Assets /> },
    { id: 'apps', icon: <Apps /> },
    { id: 'activity', icon: <Activity /> },
];
const NAVBAR_ITEMS_WITH_TEXT: NavbarItemWithId[] = [
    { id: 'home', icon: <Home />, text: 'Home' },
    { id: 'assets', icon: <Assets />, text: 'Assets' },
    { id: 'apps', icon: <Apps />, text: 'Apps' },
    { id: 'activity', icon: <Activity />, text: 'Activity' },
];

type NavbarCustomProps = NavbarProps & {
    isOpen: boolean;
};

const meta: Meta<NavbarCustomProps> = {
    component: Navbar,
    tags: ['autodocs'],
    render: () => {
        const [activeId, setActiveId] = useState<string>(NAVBAR_ITEMS[0].id);

        return (
            <div className="flex">
                <Navbar
                    items={NAVBAR_ITEMS}
                    activeId={activeId}
                    onClickItem={(id) => setActiveId(id)}
                />
            </div>
        );
    },
} satisfies Meta<typeof Navbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Collapsable: Story = {
    args: {},
    argTypes: {},
    render: (args) => {
        const [activeId, setActiveId] = useState<string>(NAVBAR_ITEMS[0].id);
        const [isOpen, setIsOpen] = useState<boolean>(false);

        return (
            <div className="flex h-96">
                <Navbar
                    isCollapsable
                    items={NAVBAR_ITEMS}
                    activeId={activeId}
                    onClickItem={(id) => setActiveId(id)}
                    isOpen={isOpen}
                    onToggleNavbar={() => setIsOpen(!isOpen)}
                />
                <NavbarSlideout
                    items={NAVBAR_ITEMS_WITH_TEXT}
                    activeId={activeId}
                    onClickItem={(id) => setActiveId(id)}
                    isOpen={isOpen}
                    onToggleNavbar={() => setIsOpen(!isOpen)}
                />
            </div>
        );
    },
};
