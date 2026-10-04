# @4da/mcp-server has moved

The 4DA MCP server now lives in its own repository, with its full history:

**https://github.com/4DA-Systems/4da-mcp-server**

Install it as before; the package name is unchanged:

```bash
claude mcp add 4da -- npx @4da/mcp-server
```

As a Claude Code plugin:

```bash
claude plugin marketplace add 4DA-Systems/4da-mcp-server
claude plugin install 4da@4da
```

This folder stays only so that older links keep resolving. Issues, releases
and contributions go to the new repository.

The server reads this app's database. The schema it relies on is published
from here as [`src-tauri/contract/app-schema.sql`](../src-tauri/contract/app-schema.sql),
generated from the migrations; CI checks every app schema change against the
server's queries before it merges.
