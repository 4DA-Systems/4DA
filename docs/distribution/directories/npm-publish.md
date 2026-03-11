# Publishing @4da/mcp-server v4.0.1 to npm

## Prerequisites
- npm account: runyourempirehq
- Build already passes (verified)

## Steps

```bash
# 1. Login to npm (one-time on this machine)
cd D:/4DA/mcp-4da-server
npm login
# Enter: runyourempirehq / password / OTP if 2FA enabled

# 2. Verify build is clean
pnpm run build

# 3. Dry run to check what will be published
npm pack --dry-run

# 4. Publish
npm publish --access public

# 5. Verify
npm view @4da/mcp-server version
# Should show: 4.0.1
```

## What Changed: 4.0.0 -> 4.0.1
Review the diff before publishing:
```bash
cd D:/4DA/mcp-4da-server
git diff HEAD -- src/ package.json
```

## Post-Publish
- Verify `npx @4da/mcp-server --setup` works with the new version
- Update all directory submissions to reference 4.0.1
- Proceed with directory submissions
