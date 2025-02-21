// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { ModuleFunction } from './ModuleFunction';
import { useNormalizedMoveModule } from '~/hooks/useNormalizedMoveModule';
import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Info, Warning } from '@iota/apps-ui-icons';

type ModuleFunctionsInteractionProps = {
    packageId: string;
    moduleName: string;
};

export function ModuleFunctionsInteraction({
    packageId,
    moduleName,
}: ModuleFunctionsInteractionProps) {
    const {
        data: normalizedModule,
        error,
        isPending,
    } = useNormalizedMoveModule(packageId, moduleName);
    const executableFunctions = useMemo(() => {
        if (!normalizedModule) {
            return [];
        }
        return Object.entries(normalizedModule.exposedFunctions)
            .filter(([_, anFn]) => anFn.isEntry)
            .map(([fnName, details]) => ({ name: fnName, details }));
    }, [normalizedModule]);
    const isEmpty = !isPending && !executableFunctions.length && !error;
    if (isEmpty || error || isPending) {
        return (
            <div className="flex h-full items-center justify-center">
                {error ? (
                    <InfoBox
                        style={InfoBoxStyle.Elevated}
                        type={InfoBoxType.Error}
                        icon={<Warning />}
                        supportingText={`Error loading module ${moduleName} details.`}
                    />
                ) : isEmpty ? (
                    <InfoBox
                        supportingText="No public entry functions found."
                        icon={<Info />}
                        type={InfoBoxType.Default}
                        style={InfoBoxStyle.Elevated}
                    />
                ) : (
                    <LoadingIndicator text="Loading data" />
                )}
            </div>
        );
    }
    return (
        <div className="flex flex-col gap-sm">
            {executableFunctions.map(({ name, details }) => (
                <ModuleFunction
                    key={name}
                    functionName={name}
                    functionDetails={details}
                    moduleName={moduleName}
                    packageId={packageId}
                />
            ))}
        </div>
    );
}
