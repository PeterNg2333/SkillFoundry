import { statSync } from 'node:fs';
import path from 'node:path';
import tseslint from 'typescript-eslint';

const crossDirectoryImports = {
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      alias: 'Use a project alias for cross-directory imports and re-exports.',
    },
  },
  create: (context) => {
    const inspect = (node) => {
      const source = node.source;
      if (typeof source?.value !== 'string') return;
      const specifier = source.value.replaceAll('\\', '/');
      if (!specifier.startsWith('.')) return;
      if (specifier === '.' || specifier === './') return;
      const directory = path.dirname(context.filename);
      const destination = path.resolve(directory, specifier);
      let crossesDirectory = path.dirname(destination) !== directory;
      if (!crossesDirectory) {
        try {
          crossesDirectory = statSync(destination).isDirectory();
        } catch (error) {
          if (error.code !== 'ENOENT' && error.code !== 'ENOTDIR') throw error;
        }
      }
      if (crossesDirectory) {
        context.report({ node: source, messageId: 'alias' });
      }
    };
    return {
      ImportDeclaration: inspect,
      ExportNamedDeclaration: inspect,
      ExportAllDeclaration: inspect,
      ImportExpression: inspect,
    };
  },
};

const createConfig = (type, configuration) => {
  const languageOptions = {
    ecmaVersion: 'latest',
    sourceType: 'module',
    parserOptions: { ecmaFeatures: { jsx: true } },
  };
  if (type === 'typescript') {
    languageOptions.parser = tseslint.parser;
    languageOptions.parserOptions.experimentalDecorators = true;
    languageOptions.parserOptions.emitDecoratorMetadata = true;
  }
  const config = [
    {
      files: ['**/*.*'],
      languageOptions,
      linterOptions: { noInlineConfig: true },
      plugins: {
        maintainable: {
          rules: { 'cross-directory-imports': crossDirectoryImports },
        },
      },
      rules: {
        'prefer-const': 'error',
        'no-var': 'error',
        'func-style': ['error', 'expression'],
        'maintainable/cross-directory-imports': 'error',
        'no-restricted-syntax': [
          'error',
          {
            selector:
              'Program > :matches(ExportNamedDeclaration, ExportDefaultDeclaration, ExportAllDeclaration) ~ :not(:matches(ExportNamedDeclaration, ExportDefaultDeclaration, ExportAllDeclaration))',
            message: 'Keep public exports together at the end of the file.',
          },
          {
            selector: 'VariableDeclarator > FunctionExpression.init',
            message: 'Use a const arrow for ordinary functions.',
          },
          {
            selector:
              'VariableDeclaration[kind!="const"] > VariableDeclarator > ArrowFunctionExpression.init',
            message: 'Bind ordinary arrows with const.',
          },
          {
            selector:
              'ExportNamedDeclaration > :matches(VariableDeclaration, FunctionDeclaration, ClassDeclaration, TSTypeAliasDeclaration, TSInterfaceDeclaration, TSEnumDeclaration)',
            message: 'Collect named value/type exports at the end.',
          },
          {
            selector:
              'ExportDefaultDeclaration > :matches(FunctionDeclaration, FunctionExpression, ArrowFunctionExpression, ClassDeclaration)',
            message:
              'Declare the value first and keep the default export at the end.',
          },
          {
            selector:
              'ExportNamedDeclaration[exportKind="value"] > ExportSpecifier[exportKind="type"]',
            message: 'Collect type exports in a separate export type list.',
          },
        ],
      },
    },
  ];
  if (type === 'typescript') {
    config.push({
      plugins: { '@typescript-eslint': tseslint.plugin },
      rules: { '@typescript-eslint/consistent-type-imports': 'error' },
    });
  }
  config.push(
    { rules: configuration.rules },
    {
      files: ['**/*.cjs', '**/*.cts'],
      languageOptions: { sourceType: 'commonjs' },
    },
    ...configuration.overrides,
  );
  return config;
};

export { createConfig };
