// Documentation side-menu structure. Every entry here MUST resolve to a real
// page under src/docs/ with a matching permalink — no dead links in the rail.
// Grouped like a docs site (the ctx.rs pattern), grounded in 4DA's own README.
export default {
  sections: [
    {
      title: "Start",
      items: [
        { url: "/docs/", label: "Overview" },
        { url: "/docs/install/", label: "Install" },
        { url: "/docs/quickstart/", label: "Quickstart" },
      ],
    },
    {
      title: "How it works",
      items: [
        { url: "/docs/how-it-works/", label: "The scoring engine" },
        { url: "/docs/sources/", label: "Sources" },
        { url: "/docs/scoring/", label: "The 5 axes" },
        { url: "/docs/privacy/", label: "Privacy & BYOK" },
      ],
    },
    {
      title: "Integrate",
      items: [
        { url: "/docs/mcp/", label: "MCP server & agents" },
        { url: "/docs/cli/", label: "CLI" },
      ],
    },
  ],
};
