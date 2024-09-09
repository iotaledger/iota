// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const IOTA_COPYRIGHT_HEADER = 'Copyright (c) 2024 IOTA Stiftung';
const OLD_COPYRIGHT_HEADER = 'Copyright (c) Mysten Labs, Inc.';
const MODIFICATION_COPYRIGHT_HEADER = 'Modifications Copyright (c) 2024 IOTA Stiftung';
const LICENSE_IDENTIFIER = 'SPDX-License-Identifier: Apache-2.0';

const MISSING_HEADER_MESSAGE = 'Missing or incorrect license header.';
const MISSING_MODIFICATION_MESSAGE = 'Add modification notice to the license header.';

const IOTA_LICENSE_HEADER = `// ${IOTA_COPYRIGHT_HEADER}\n// ${LICENSE_IDENTIFIER}\n\n`;

function checkHeader(node, context) {
    const sourceCode = context.getSourceCode();
    const comments = sourceCode.getAllComments();
    const firstComment = comments?.[0]?.value;

    const hasIotaCopyrightHeader = firstComment?.includes(IOTA_COPYRIGHT_HEADER);
    const hasOldCopyrightHeader = firstComment?.includes(OLD_COPYRIGHT_HEADER);

    // Check if the file has any license header.
    if ((!hasIotaCopyrightHeader && !hasOldCopyrightHeader) || !firstComment) {
        context.report({
            node,
            message: MISSING_HEADER_MESSAGE,
            fix(fixer) {
                return fixer.insertTextBeforeRange([0, 0], IOTA_LICENSE_HEADER);
            },
        });

        // Check if the file has the old copyright notice and has the modification header.
    } else if (firstComment.includes(OLD_COPYRIGHT_HEADER)) {
        const hasModificationNotice = comments[1]?.value?.includes(MODIFICATION_COPYRIGHT_HEADER);
        if (!hasModificationNotice) {
            context.report({
                node: comments[0],
                message: MISSING_MODIFICATION_MESSAGE,
                fix(fixer) {
                    return fixer.insertTextAfter(
                        comments[0],
                        `\n// ${MODIFICATION_COPYRIGHT_HEADER}`,
                    );
                },
            });
        }
    }
}

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
        return {
            Program(node) {
                checkHeader(node, context);
            },
        };
    },
};
