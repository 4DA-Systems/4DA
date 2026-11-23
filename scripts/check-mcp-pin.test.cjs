// Every place in this repository that states an @4da/mcp-server CONFIGURATION (what an MCP
// client runs at every start) names one version, the one in site/src/_data/mcpServer.js, and
// runs it as `npx -y --prefer-offline @4da/mcp-server@<version>`.
//
// Why pinned: an unpinned `npx @4da/mcp-server` keeps whatever version npx fetched first, with
// no sign that it is old. Pinned, the same server starts every time and --prefer-offline starts
// it without asking the registry first; from 6.5.0 the server says when a newer release is
// published and names the command that moves the pin.
//
// After each @4da/mcp-server release: set VERSION in the data file and run this test. It fails
// until README.md, mcp-4da-server/README.md, gemini-extension.json and site/src/docs/mcp.md agree.
//
// Deliberately not held here:
// - one-off commands a person types once (`npx @4da/mcp-server` on the home and download pages,
//   `npx -y @4da/mcp-server@latest --setup`): they are not configurations;
// - src/lib/agent-handoff.ts MCP_SETUP_LINE: it is compiled into the app, and a version there
//   would be wrong in every installed build once the next server release is out;
// - .mcp.json: this repository's own development configuration, which follows @latest.
'use strict';

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(root, file), 'utf8');

const DATA_FILE = 'site/src/_data/mcpServer.js';
const VERSION = (/^const VERSION = "(\d+\.\d+\.\d+)";$/m.exec(read(DATA_FILE)) || [])[1];
const SPEC = `@4da/mcp-server@${VERSION}`;

/** Files outside the site: they cannot read the data file, so they state the version. */
const LITERAL_FILES = ['README.md', 'mcp-4da-server/README.md'];
/** The docs pages are plain Markdown (templateEngineOverride: md), so this one states it too. */
const DOCS_CONFIG_FILE = 'site/src/docs/mcp.md';
/** Site templates: the version comes from the data file. */
const TEMPLATE_FILES = ['site/src/mcp.njk', 'site/src/index.njk'];
/** Every file a reader may copy a configuration from. */
const ALL_FILES = [...LITERAL_FILES, DOCS_CONFIG_FILE, ...TEMPLATE_FILES, 'gemini-extension.json', 'site/src/download.njk', 'site/src/docs/install.md', 'site/src/docs/index.md'];

/** Versions `text` pins that are not the data file's. */
const otherVersions = (text) => [...text.matchAll(/@4da\/mcp-server@(\d+\.\d+\.\d+)/g)].map((m) => m[1]).filter((v) => v !== VERSION);

test('the data file names one released version', () => {
  assert.match(String(VERSION), /^\d+\.\d+\.\d+$/, `${DATA_FILE}: expected a line \`const VERSION = "X.Y.Z";\``);
});

test('README.md and the stub README give the pinned Claude Code command, and no other version', () => {
  for (const file of LITERAL_FILES) {
    const text = read(file);
    assert.ok(
      text.includes(`claude mcp add --scope user 4da -- npx -y --prefer-offline ${SPEC}`),
      `${file}: the Claude Code command must be \`claude mcp add --scope user 4da -- npx -y --prefer-offline ${SPEC}\``,
    );
    const others = otherVersions(text);
    assert.deepEqual(others, [], `${file}: names @4da/mcp-server@${others[0]}, but ${DATA_FILE} says ${VERSION}`);
  }
});

test('the docs page gives the pinned configuration, and no other version', () => {
  const text = read(DOCS_CONFIG_FILE);
  assert.ok(text.includes(`"args": ["-y", "--prefer-offline", "${SPEC}"]`), `${DOCS_CONFIG_FILE}: the configuration's args must be ["-y", "--prefer-offline", "${SPEC}"]`);
  assert.doesNotMatch(text, /\{\{/, `${DOCS_CONFIG_FILE}: this page is not templated, so a {{ variable }} would be shown as written`);
  const others = otherVersions(text);
  assert.deepEqual(others, [], `${DOCS_CONFIG_FILE}: names @4da/mcp-server@${others[0]}, but ${DATA_FILE} says ${VERSION}`);
});

test('gemini-extension.json runs the pinned server and carries its version', () => {
  const extension = JSON.parse(read('gemini-extension.json'));
  assert.deepEqual(extension.mcpServers['4da'].args, ['-y', '--prefer-offline', SPEC]);
  assert.equal(extension.version, VERSION);
});

test('site templates take the version from the data file, never a literal', () => {
  for (const file of TEMPLATE_FILES) {
    const text = read(file);
    assert.ok(text.includes('--prefer-offline'), `${file}: its configuration must start npx with --prefer-offline`);
    assert.ok(text.includes('{{ mcpServer.spec }}'), `${file}: its configuration must name {{ mcpServer.spec }}`);
    assert.doesNotMatch(text, /@4da\/mcp-server@\d/, `${file}: a literal version; use {{ mcpServer.spec }} so ${DATA_FILE} stays the one place`);
  }
});

test('no configuration is left unpinned', () => {
  // A JSON args list that ends in the bare package name (the home page wraps strings in spans),
  // or a `claude mcp add` command that does.
  const unpinned = [/"@4da\/mcp-server"(?:<\/span>)?\s*\]/, /claude mcp add[^\n<]*@4da\/mcp-server(?![@\w/-])/];
  for (const file of ALL_FILES) {
    const text = read(file);
    for (const pattern of unpinned) {
      const found = pattern.exec(text);
      assert.equal(found, null, `${file}: an unpinned configuration (${found && found[0]}); pin it to ${SPEC} with --prefer-offline`);
    }
  }
});
