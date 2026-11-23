// The @4da/mcp-server release that this site's configuration snippets pin.
//
// A configuration is what an MCP client runs at every start. Pinned, it starts
// the same server each time and, with npx's --prefer-offline, without asking
// the registry first; unpinned, npx keeps whatever version it fetched first,
// silently. The server itself says when a newer release is published
// (get_context and --doctor), with the command that moves the pin.
//
// After each @4da/mcp-server release: set VERSION here, then run
// `node --test scripts/check-mcp-pin.test.cjs`. It names every file that states
// the version itself (README.md, gemini-extension.json, mcp-4da-server/README.md
// and docs/mcp.md, which is plain Markdown) and fails until they agree.
const VERSION = "6.5.0";

export default {
  version: VERSION,
  // The npx package spec, as a configuration's last argument.
  spec: `@4da/mcp-server@${VERSION}`,
};
