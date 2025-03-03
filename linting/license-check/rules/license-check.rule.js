// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const CURRENT_YEAR = new Date().getFullYear();
const EARLIEST_VALID_YEAR = 2024;

const OLD_COPYRIGHT_HEADER = 'Copyright (c) Mysten Labs, Inc.';
const LICENSE_IDENTIFIER = 'SPDX-License-Identifier: Apache-2.0';

const MISSING_HEADER_MESSAGE = 'Missing or incorrect license header.';
const MISSING_MODIFICATION_MESSAGE = 'Add modification notice to the license header.';


function createCopyrightYearPattern() {
    const years = Array.from({ length: CURRENT_YEAR - EARLIEST_VALID_YEAR + 1 }, (_, i) => i + EARLIEST_VALID_YEAR);
    return {
        header: new RegExp(`Copyright \\(c\\) (${years.join('|')}) IOTA Stiftung`),
        modificationHeader: new RegExp(`Modifications Copyright \\(c\\) (${years.join('|')}) IOTA Stiftung`)
    }
};

const { header: copyrightPattern, modificationHeader: modificationCopyrightHeader } = createCopyrightYearPattern();

const licenseHeader = `// Copyright (c) ${CURRENT_YEAR} IOTA Stiftung\n// ${LICENSE_IDENTIFIER}\n\n`;
const modificationsComment = `// Modifications Copyright (c) ${CURRENT_YEAR} IOTA Stiftung`;

function checkHeader(node, context) {
    const sourceCode = context.getSourceCode();
    const comments = sourceCode.getAllComments();
    const firstComment = comments?.[0]?.value;

    const hasValidIotaCopyrightHeader = firstComment && copyrightPattern.test(firstComment);
    const hasOldCopyrightHeader = firstComment?.includes(OLD_COPYRIGHT_HEADER);

    // Check if the file has the correct license header.
    if (firstComment && !hasValidIotaCopyrightHeader && !hasOldCopyrightHeader) {
        return context.report({
            node: comments[0],
            message: 'Invalid license header.',
        });
    }

    // Check if the file has any license header.
    if ((!hasValidIotaCopyrightHeader && !hasOldCopyrightHeader) || !firstComment) {
        return context.report({
            node,
            message: MISSING_HEADER_MESSAGE,
            fix(fixer) {
                return fixer.insertTextBeforeRange([0, 0], licenseHeader);
            },
        });

        // Check if the file has the old copyright notice and has the modification header.
    } else if (hasOldCopyrightHeader) {
        const modificationComment = comments[1]?.value;
        const hasModificationHeader = modificationComment && modificationCopyrightHeader.test(modificationComment);
        if (!hasModificationHeader) {
            return context.report({
                node: comments[0],
                message: MISSING_MODIFICATION_MESSAGE,
                fix(fixer) {
                    return fixer.insertTextAfter(
                        comments[0],
                        `\n${modificationsComment}`,
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
