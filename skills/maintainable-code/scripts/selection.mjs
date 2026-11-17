import { execFile } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const execute = promisify(execFile);
const binary = fileURLToPath(
  new URL(
    process.platform === 'win32'
      ? '../bin/maintainable-check.exe'
      : '../bin/maintainable-check',
    import.meta.url,
  ),
);

const selectFiles = async (options) => {
  const arguments_ = [
    'select',
    '--root',
    path.resolve(options.root ?? process.cwd()),
    '--json',
  ];
  if (options.config) arguments_.push('--config', options.config);
  arguments_.push('--', ...(options.paths ?? []));
  try {
    const { stdout } = await execute(binary, arguments_, {
      maxBuffer: 16 * 1024 * 1024,
      windowsHide: true,
    });
    return JSON.parse(stdout);
  } catch (error) {
    throw new Error(
      error.stderr?.trim() ||
        `Could not run the supplied Go selector. ${error.message}`,
      { cause: error },
    );
  }
};

export { selectFiles };
