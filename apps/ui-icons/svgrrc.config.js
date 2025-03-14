const path = require('path');

/** @type {import('@svgr/core').Config} */
module.exports = {
    // The default parser set by svgr is `babel`, which makes the import sorting plugin fail.
    prettierConfig: {
        parser: 'babel-ts',
    },
    icon: true,
    typescript: true,
    outDir: './src',
    jsxRuntime: 'automatic',
    replaceAttrValues: {
        '#171D26': 'currentColor',
    },
    indexTemplate(filePaths) {
        const exportEntries = filePaths.map((filePath) => {
            const basename = path.basename(filePath, path.extname(filePath));
            const exportName = /^\d/.test(basename) ? `Svg${basename}` : basename;
            return `export { default as ${exportName} } from './${basename}'`;
        });
        return exportEntries.join('\n');
    },
    template(variables, { tpl }) {
        const template = tpl`

        ${variables.imports};

    ${variables.interfaces};

    export default function ${variables.componentName}(${variables.props}) {
        return (
            ${variables.jsx}
        )
    };
    `;

        return template;
    },
};
