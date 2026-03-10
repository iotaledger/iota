// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface DropdownMenuOptionProps {
    icon: React.ReactNode;
    label: string;
}

export function DropdownMenuOption({ icon, label }: DropdownMenuOptionProps) {
    return (
        <div className="flex items-center gap-sm min-w-[228px] text-body-lg text-names-neutral-92">
            <span className="[&_svg]:h-6 [&_svg]:w-6">{icon}</span>
            <span>{label}</span>
        </div>
    );
}
