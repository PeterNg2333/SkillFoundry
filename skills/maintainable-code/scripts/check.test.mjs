import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

const checker = path.join(import.meta.dirname, 'check.mjs');

const withProject = async (files, verify) => {
  const prefix = path.join(await realpath(tmpdir()), 'maintainable-check-');
  const root = await mkdtemp(prefix);
  try {
    for (const [name, source] of Object.entries(files)) {
      const file = path.join(root, name);
      await mkdir(path.dirname(file), { recursive: true });
      await writeFile(file, source);
    }
    await verify(root);
  } finally {
    const resolved = await realpath(root);
    assert.ok(resolved.startsWith(prefix));
    await rm(resolved, { recursive: true, force: true });
  }
};

const run = (root, arguments_ = []) => {
  const result = spawnSync(
    process.execPath,
    [checker, '--root', root, '--json', ...arguments_],
    { cwd: tmpdir(), encoding: 'utf8' },
  );
  if (result.error) throw result.error;
  return {
    ...result,
    report: result.stdout ? JSON.parse(result.stdout) : undefined,
  };
};

test('runs supplied JS/TS/JSX/JSON checks in a project without lint dependencies', async () => {
  await withProject(
    {
      'value.ts':
        'type Label = string; class Model { label(): Label { return "ok"; } } const value = new Model().label(); export { Model, value }; export type { Label };',
      'Page.tsx': 'const Page = () => <p>ok</p>; export default Page;',
      'Page.jsx': 'const Page = () => <p>ok</p>; export default Page;',
      'module.mjs': 'const value = 1; export { value };',
      'module.cjs': 'const value = 1; module.exports = { value };',
      'di.ts':
        'import { Injectable } from "@nestjs/common"; import { Service } from "@/Service"; @Injectable() class Model { constructor(private readonly service: Service) {} } export { Model };',
      'overload.ts':
        'function choose(value: string): string; function choose(value: number): number; function choose(value: string | number) { return value; } export { choose };',
      'settings.json': '\uFEFF{"enabled":true}',
      'eslint.config.mjs':
        'throw new Error("Project configuration must not execute");',
    },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.report.summary.checked, 9);
      assert.deepEqual(result.report.diagnostics, []);
    },
  );
});

test('reports style, JS syntax and JSON syntax errors through the CLI', async () => {
  await withProject(
    {
      'style.ts': 'export function choose() { return 1; }',
      'exports.ts':
        'const first = 1; export { first }; const second = 2; export { second };',
      'syntax.js': 'const value: number = 1;',
      'broken.json': '\uFEFF{\n"enabled":\n}',
      'comment.json': '{/* comment */"enabled":true}',
      'trailing.json': '{"enabled":true,}',
    },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 1, result.stderr);
      const rules = result.report.diagnostics.map((message) => message.rule);
      assert.ok(rules.includes('func-style'));
      assert.ok(rules.includes('no-restricted-syntax'));
      assert.ok(
        result.report.diagnostics.some(
          (message) =>
            message.file === 'exports.ts' &&
            message.message ===
              'Keep public exports together at the end of the file.',
        ),
      );
      assert.ok(rules.includes('syntax'));
      assert.ok(rules.includes('json/syntax'));
      const jsonError = result.report.diagnostics.find(
        (message) => message.file === 'broken.json',
      );
      assert.equal(jsonError.line, 3);
      assert.equal(jsonError.column, 1);
      for (const file of ['comment.json', 'trailing.json']) {
        assert.ok(
          result.report.diagnostics.some(
            (message) =>
              message.file === file && message.rule === 'json/syntax',
          ),
        );
      }
    },
  );
});

test('applies ignore globs and extension routing before parsing', async () => {
  await withProject(
    {
      'checkignore.json': JSON.stringify({
        ignore: ['ignored/**'],
        extensions: {
          '.js': false,
          '.source': 'typescript',
          '.generated.ts': false,
        },
      }),
      'ignored/.hidden.ts': 'broken !!!',
      'node_modules/vendor/broken.ts': 'broken !!!',
      'disabled.js': 'broken !!!',
      'view.generated.ts': 'broken !!!',
      'unsupported.py': 'broken !!!',
      'custom.source': 'type Label = string; export type { Label };',
      'upper.TS': 'const value = 1; export { value };',
    },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(result.report.diagnostics, []);
      assert.deepEqual(result.report.files.map((file) => file.path).sort(), [
        'checkignore.json',
        'custom.source',
        'upper.TS',
      ]);
      assert.ok(
        result.report.skipped.some(
          (file) => file.path === 'ignored' && file.reason === 'ignore',
        ),
      );
      assert.ok(
        result.report.skipped.some(
          (file) => file.path === 'node_modules' && file.reason === 'ignore',
        ),
      );
      assert.ok(
        result.report.skipped.some(
          (file) =>
            file.path === 'disabled.js' && file.reason === 'disabled-extension',
        ),
      );
      assert.ok(
        result.report.skipped.some(
          (file) =>
            file.path === 'view.generated.ts' &&
            file.reason === 'disabled-extension',
        ),
      );
      assert.ok(
        result.report.skipped.some(
          (file) =>
            file.path === 'unsupported.py' &&
            file.reason === 'unsupported-extension',
        ),
      );
    },
  );
});

test('checks parent, child-directory and dynamic imports while allowing same-directory imports', async () => {
  await withProject(
    {
      'shared.ts': 'type Shared = string; export type { Shared };',
      'src/peer.ts': 'type Peer = string; export type { Peer };',
      'src/index.ts': 'const value = 1; export { value };',
      'src/sameIndex.ts': 'export { value } from "./";',
      'src/nested/index.ts': 'const nested = 1; export { nested };',
      'src/same.ts':
        'import type { Peer } from "./peer"; export type { Peer };',
      'src/parent.ts': 'export type { Shared } from "../shared";',
      'src/child.ts': 'import "./nested/helper";',
      'src/childIndex.ts': 'import "./nested";',
      'src/dynamic.ts':
        'const load = () => import("../shared"); export { load };',
    },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 1, result.stderr);
      const failures = result.report.diagnostics
        .filter(
          (message) => message.rule === 'maintainable/cross-directory-imports',
        )
        .map((message) => message.file)
        .sort();
      assert.deepEqual(failures, [
        'src/child.ts',
        'src/childIndex.ts',
        'src/dynamic.ts',
        'src/parent.ts',
      ]);
      assert.ok(
        result.report.diagnostics.every(
          (message) =>
            !['src/same.ts', 'src/sameIndex.ts'].includes(message.file),
        ),
      );
    },
  );
});

test('returns warning locations without failing and preserves metric boundaries', async () => {
  const branches = Array.from(
    { length: 10 },
    (_, index) => `if (value === ${index}) return ${index};`,
  );
  await withProject(
    {
      'nested.ts':
        'const choose = (a: boolean, b: boolean, c: boolean) => { if (a) { if (b) { if (c) return 1; } } return 0; }; export { choose };',
      'complex.ts': `const choose = (value: number) => { ${branches.join('\n')} return -1; }; export { choose };`,
      'boundary.ts': `const choose = (value: number) => { ${branches.slice(0, 9).join('\n')} return -1; }; export { choose };`,
      'long.mjs': 'console.info("review");\n'.repeat(301),
      'boundary.mjs': `${'// context\n'.repeat(10)}\n${'console.info("review");\n'.repeat(300)}`,
    },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.report.summary.errors, 0);
      for (const rule of ['max-depth', 'complexity', 'max-lines']) {
        assert.ok(
          result.report.diagnostics.some(
            (message) =>
              message.rule === rule &&
              message.severity === 'warning' &&
              message.line >= 1,
          ),
        );
      }
      assert.ok(
        result.report.diagnostics.every(
          (message) => !message.file.startsWith('boundary.'),
        ),
      );
    },
  );
});

test('uses explicit rule exceptions from checkignore and keeps inline disables visible', async () => {
  await withProject(
    {
      'checkignore.json': JSON.stringify({
        overrides: [{ files: ['fixture.js'], rules: { 'func-style': 'off' } }],
      }),
      'fixture.js': 'function choose() { return 1; } export { choose };',
      'source.js':
        '/* eslint-disable func-style */ function choose() { return 1; } export { choose };',
    },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 1, result.stderr);
      assert.ok(
        result.report.diagnostics.every(
          (message) => message.file !== 'fixture.js',
        ),
      );
      assert.ok(
        result.report.diagnostics.some(
          (message) =>
            message.file === 'source.js' && message.rule === 'func-style',
        ),
      );
    },
  );
});

test('lists selection without claiming a check and limits explicit paths', async () => {
  await withProject(
    { 'first.ts': 'broken !!!', 'second.ts': 'broken !!!' },
    (root) => {
      const result = run(root, ['--list', 'first.ts']);
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.report.mode, 'list');
      assert.equal(result.report.summary.selected, 1);
      assert.equal(result.report.summary.checked, 0);
      assert.deepEqual(result.report.diagnostics, []);
      assert.equal(run(root, ['../outside.ts']).status, 2);
    },
  );
});

test('fails clearly on malformed configuration and unsupported checker types', async () => {
  await withProject({ 'checkignore.json': '{' }, (root) => {
    const result = run(root);
    assert.equal(result.status, 2);
    assert.match(result.stderr, /checkignore\.json/);
  });
  await withProject(
    { 'checkignore.json': JSON.stringify({ extensions: { '.py': 'python' } }) },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 2);
      assert.match(result.stderr, /Unsupported extension mapping/);
    },
  );
});

test('shares Git and Docker ignore selection before JS/TS parsing', async () => {
  await withProject(
    {
      '.gitignore': 'git-skipped/\n*.secret.ts\n!keep.secret.ts\n',
      '.dockerignore': 'docker-skipped/\n!docker-skipped/keep.ts\n',
      'git-skipped/bad.ts': 'invalid !!!',
      'bad.secret.ts': 'invalid !!!',
      'keep.secret.ts': 'const value = 1; export { value };',
      'docker-skipped/bad.ts': 'invalid !!!',
      'docker-skipped/keep.ts': 'const value = 1; export { value };',
    },
    (root) => {
      const result = run(root);
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.report.summary.checked, 2);
      assert.deepEqual(result.report.diagnostics, []);
      assert.ok(
        result.report.skipped.some((file) => file.reason === 'gitignore'),
      );
      assert.ok(
        result.report.skipped.some((file) => file.reason === 'dockerignore'),
      );
    },
  );
});
