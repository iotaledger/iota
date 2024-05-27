// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
const licenseIdentifier = 'SPDX-License-Identifier: Apache-2.0'
const oldHeader = 'Copyright (c) Mysten Labs, Inc.'
const newHeader = 'Copyright (c) 2024 IOTA Stiftung'
const modificationNotice = 'Modifications Copyright (c) 2024 IOTA Stiftung'

module.exports = {
    meta: {
        type: 'suggestion',
        docs: {
            description: 'Check and fix license header',
            category: 'Stylistic Issues',
        },
        fixable: 'code',
        schema: [],
    },
    create(context) {
        const sourceCode = context.getSourceCode();

        function checkHeader(node) {
            const comments = sourceCode.getAllComments();
            const firstFourComments = comments.slice(0, 4) ?? [];
            const copyrightHeader = firstFourComments?.[0]?.value;

            const hasCorrectHeader =
                copyrightHeader && copyrightHeader.includes('Copyright (c) 2024 IOTA Stiftung');
            const hasOldHeader =
                copyrightHeader && copyrightHeader.includes('Copyright (c) Mysten Labs, Inc.');

            if ((!hasCorrectHeader && !hasOldHeader) || !copyrightHeader) {
                context.report({
                    node,
                    message: 'Missing or incorrect license header.',
                    fix(fixer) {
                        return fixer.insertTextBeforeRange(
                            [0, 0],
                            `// ${newHeader}\n// ${licenseIdentifier}\n\n`,
                        );
                    },
                });
            } else if (copyrightHeader.includes(oldHeader)) {
                const hasModificationNotice =
                    firstFourComments[2]?.value?.includes(modificationNotice);
                if (!hasModificationNotice) {
                    context.report({
                        node: firstFourComments[1],
                        message: 'Add modification notice to the license header.',
                        fix(fixer) {
                            return fixer.insertTextAfter(
                                firstFourComments[1],
                                `\n\n// ${modificationNotice}\n// ${licenseIdentifier}\n\n`,
                            );
                        },
                    });
                }
            }
        }

        return {
            Program(node) {
                checkHeader(node);
            },
        };
    },
};
