import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { ESLint } from 'eslint';
import { parse as parseJson, printParseErrorCode } from 'jsonc-parser';
import { createConfig } from './rules.mjs';
import { selectFiles } from './selection.mjs';

const checkJson = (source, file) => {
  const text = source.replace(/^\uFEFF/, '');
  const errors = [];
  parseJson(text, errors, {
    disallowComments: true,
    allowTrailingComma: false,
    allowEmptyContent: false,
  });
  return errors.map((error) => {
    const lines = text.slice(0, error.offset).split('\n');
    return {
      file,
      line: lines.length,
      column: lines.at(-1).length + 1,
      severity: 'error',
      rule: 'json/syntax',
      origin: 'static',
      message: `Invalid JSON syntax (${printParseErrorCode(error.error)}).`,
    };
  });
};

const runCheck = async (options = {}) => {
  const selected = await selectFiles(options);
  const root = selected.root;
  const configuration = selected.configuration;
  const selection = { files: selected.files, skipped: [...selected.skipped] };
  const supportedTypes = new Set(['javascript', 'typescript', 'json']);
  const candidates = selection.files.filter((file) =>
    supportedTypes.has(file.type),
  );
  const engines = new Map();
  const diagnostics = [];
  if (!options.list) {
    for (const file of selection.files) {
      if (!supportedTypes.has(file.type)) {
        selection.skipped.push({
          path: file.path,
          reason: 'native-check-required',
        });
      }
    }
    for (const file of candidates) {
      const absolute = path.join(root, file.path);
      const source = await readFile(absolute, 'utf8');
      if (file.type === 'json') {
        diagnostics.push(...checkJson(source, file.path));
        continue;
      }
      if (!engines.has(file.type)) {
        engines.set(
          file.type,
          new ESLint({
            cwd: root,
            overrideConfigFile: true,
            ignore: false,
            overrideConfig: createConfig(file.type, configuration),
          }),
        );
      }
      const results = await engines
        .get(file.type)
        .lintText(source, { filePath: absolute });
      if (results.length !== 1)
        throw new Error(`No checker result for ${file.path}`);
      for (const message of results[0].messages) {
        diagnostics.push({
          file: file.path,
          line: message.line ?? 1,
          column: message.column ?? 1,
          severity: message.severity === 2 ? 'error' : 'warning',
          rule: message.ruleId ?? 'syntax',
          origin: 'static',
          message: message.message,
        });
      }
    }
  }
  return {
    schemaVersion: 1,
    root,
    config: {
      defaults: configuration.defaults,
      project: configuration.project,
    },
    mode: options.list ? 'list' : 'check',
    ...selection,
    diagnostics,
    summary: {
      selected: selection.files.length,
      checked: options.list ? 0 : candidates.length,
      unchecked: options.list ? 0 : selection.files.length - candidates.length,
      skipped: selection.skipped.length,
      errors: diagnostics.filter((message) => message.severity === 'error')
        .length,
      warnings: diagnostics.filter((message) => message.severity === 'warning')
        .length,
    },
  };
};

const parseArguments = (arguments_) => {
  const options = { paths: [] };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === '--root' || argument === '--config') {
      const value = arguments_[index + 1];
      if (!value || value.startsWith('--'))
        throw new Error(`Expected a value after ${argument}`);
      options[argument.slice(2)] = value;
      index += 1;
    } else if (['--json', '--list', '--help'].includes(argument)) {
      options[argument.slice(2)] = true;
    } else {
      if (argument.startsWith('--'))
        throw new Error(`Unknown option ${argument}`);
      options.paths.push(argument);
    }
  }
  return options;
};

const main = async () => {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) {
    console.log(
      'node check.mjs [--root directory] [--config file] [--json] [--list] [relative paths...]',
    );
    return;
  }
  const result = await runCheck(options);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    if (options.list) {
      for (const file of result.files) console.log(`${file.type} ${file.path}`);
    }
    for (const message of result.diagnostics) {
      console.log(
        `${message.file}:${message.line}:${message.column} ${message.severity} ${message.rule} ${message.message}`,
      );
    }
    console.log(
      `${result.summary.selected} selected, ${result.summary.checked} checked, ${result.summary.skipped} paths skipped, ${result.summary.errors} errors, ${result.summary.warnings} warnings`,
    );
  }
  process.exitCode = result.summary.errors > 0 ? 1 : 0;
};

if (
  process.argv[1] &&
  pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url
) {
  main().catch((error) => {
    console.error(`Static check failed. ${error.message}`);
    process.exitCode = 2;
  });
}

export { runCheck };
