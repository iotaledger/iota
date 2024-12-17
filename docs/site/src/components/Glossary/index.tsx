import React from 'react';
import Heading from '@theme/Heading';
import { toTitleCase } from '@artsy/to-title-case';
import { clsx } from 'clsx';
import parse from 'html-react-parser';

export default function Glossary() {
    const glossary = require('@site/config/jargon.js');

    const sortedGlossary = Object.keys(glossary)
        .sort(function (a, b) {
            return a.toLowerCase().localeCompare(b.toLowerCase());
        })
        .reduce((acc, key) => {
            acc[key] = glossary[key];
            return acc;
        }, {});

    let char = '';
    return (
        <>
            {Object.entries(sortedGlossary).map(([key, value]) => {
                let heading = null;
                if (key.charAt(0).toLowerCase() !== char.toLowerCase()) {
                    char = key.charAt(0);
                    heading = char;
                }

                return (
                    <>
                        {heading && (
                            <Heading
                                as='h2'
                                id={char}
                                title={char}
                            >
                                {char.toUpperCase()}
                            </Heading>
                        )}
                        <Heading
                            as='h3'
                            id={key}
                            title={key}
                        >
                            {toTitleCase(key)}
                        </Heading>
                        <p>{parse(value)}</p>
                    </>
                );
            })}
        </>
    );
}