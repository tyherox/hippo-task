# HippoTask — Distribution & Deployment

> How HippoTask is packaged, published, and consumed across different environments.

---

## Table of Contents

- [1. Distribution Model](#1-distribution-model)
- [2. Package Publishing](#2-package-publishing)
- [3. Multi-Environment Support](#3-multi-environment-support)
- [4. MCP Server Distribution](#4-mcp-server-distribution)
- [5. JSON Schema Distribution](#5-json-schema-distribution)
- [6. Documentation Site](#6-documentation-site)
- [7. Release Process](#7-release-process)
- [8. Versioning Strategy](#8-versioning-strategy)

---

## 1. Distribution Model

HippoTask is distributed as **multiple small npm packages** under the `@hippotask` scope. This is the primary distribution channel — no hosted service, no Docker image (for the library), no SaaS.

```
End User Installs:
───────────────────────────────────────────────────

Minimal (schema only):        npm install @hippotask/core
                              → 0 external deps (just zod)
                              → ~15KB minified

With one adapter:             npm install @hippotask/core @hippotask/adapter-jira
                              → Jira-specific deps only

Full MCP setup:               npm install @hippotask/core @hippotask/mcp-server @hippotask/adapter-jira @hippotask/adapter-linear
                              → MCP SDK + selected adapter deps

Just the JSON Schema:         curl https://unpkg.com/@hippotask/core/schema/hippo-task.schema.json
                              → Zero install, any language
```

### Why npm (and Not Something Else)?

| Channel | Why / Why Not |
|---------|---------------|
| **npm** ✅ | Primary. TypeScript packages live here. Broadest reach for JS/TS developers. |
| **JSR (Deno)** ✅ | Secondary. Publish to JSR for Deno users. Same source, different registry. |
| **unpkg / jsdelivr CDN** ✅ | Auto-available via npm. Enables `<script>` tag usage for core schema in browsers. |
| **GitHub Packages** ❌ | Redundant with npm. Adds configuration complexity for consumers. |
| **PyPI** ⏳ (future) | If demand exists, publish JSON Schema + Python validation wrapper. Not day-one. |
| **Docker** ⏳ (future) | For the MCP server *only* — as a standalone container for agent environments. Not day-one. |
| **Homebrew / apt** ❌ | Too niche. CLI users can use `npx`. |

---

## 2. Package Publishing

### Package Matrix

| Package | npm Scope | Size Target | External Deps |
|---------|-----------|-------------|---------------|
| `@hippotask/core` | `@hippotask/core` | < 20KB min+gz | `zod` only |
| `@hippotask/adapter-common` | `@hippotask/adapter-common` | < 10KB min+gz | `@hippotask/core` |
| `@hippotask/adapter-jira` | `@hippotask/adapter-jira` | < 30KB min+gz | `@hippotask/core`, `adapter-common`, ADF converter |
| `@hippotask/adapter-linear` | `@hippotask/adapter-linear` | < 20KB min+gz | `@hippotask/core`, `adapter-common` |
| `@hippotask/adapter-github` | `@hippotask/adapter-github` | < 20KB min+gz | `@hippotask/core`, `adapter-common`, `@octokit/graphql` |
| `@hippotask/adapter-asana` | `@hippotask/adapter-asana` | < 25KB min+gz | `@hippotask/core`, `adapter-common`, `turndown`, `marked` |
| `@hippotask/adapter-clickup` | ... | < 20KB | ... |
| `@hippotask/adapter-trello` | ... | < 20KB | ... |
| `@hippotask/adapter-notion` | ... | < 25KB | ... Notion block converters |
| `@hippotask/adapter-monday` | ... | < 20KB | ... |
| `@hippotask/adapter-todoist` | ... | < 15KB | ... |
| `@hippotask/adapter-planner` | ... | < 20KB | ... Graph SDK |
| `@hippotask/mcp-server` | `@hippotask/mcp-server` | < 30KB min+gz | `@hippotask/core`, `adapter-common`, `@modelcontextprotocol/sdk` |

### Build Output

Each package is built with `tsup` and produces:

```
dist/
├── index.mjs          # ESM (primary)
├── index.cjs          # CJS (fallback for older Node.js/tooling)
├── index.d.ts         # TypeScript declarations
└── index.d.mts        # ESM TypeScript declarations
```

### package.json Exports

```jsonc
{
  "name": "@hippotask/core",
  "type": "module",
  "exports": {
    ".": {
      "import": {
        "types": "./dist/index.d.mts",
        "default": "./dist/index.mjs"
      },
      "require": {
        "types": "./dist/index.d.ts",
        "default": "./dist/index.cjs"
      }
    },
    "./schema/hippo-task.schema.json": "./schema/hippo-task.schema.json",
    "./schema/hippo-project.schema.json": "./schema/hippo-project.schema.json"
  },
  "files": ["dist", "schema", "README.md", "LICENSE"],
  "engines": { "node": ">=20" },
  "sideEffects": false
}
```

---

## 3. Multi-Environment Support

### Environment Compatibility Matrix

| Environment | `@hippotask/core` | Adapters | MCP Server |
|-------------|-------------------|----------|------------|
| **Node.js 20+** | ✅ Full support | ✅ Full support | ✅ Full support |
| **Node.js 18** | ✅ Full support | ✅ Full support | ⚠️ May need `--experimental-fetch` |
| **Deno** | ✅ Via npm: specifier | ✅ Via npm: specifier | ✅ stdio transport |
| **Bun** | ✅ Full support | ✅ Full support | ✅ Full support |
| **Browser (modern)** | ✅ Schema validation only | ❌ No server-side APIs | ❌ No server-side |
| **Cloudflare Workers** | ✅ Schema validation | ⚠️ Limited (no long connections) | ❌ No stdio |
| **Vercel Edge** | ✅ Schema validation | ⚠️ Limited | ❌ No stdio |
| **AWS Lambda** | ✅ Full | ✅ Full (with timeout awareness) | ⚠️ HTTP transport only |

### How This Works

**`@hippotask/core`** has zero Node.js-specific APIs. It's pure TypeScript/JavaScript:
- Zod schemas (platform-independent)
- ID generation (uses `crypto.randomUUID()` — available everywhere)
- Date utilities (standard `Date` + string manipulation)
- Validation logic (pure functions)

This means `@hippotask/core` works **everywhere JavaScript runs** — browsers, edge functions, service workers, React Native, Electron.

**Adapters** require HTTP capabilities (`fetch`). They work in any environment with `globalThis.fetch` (Node.js 18+, Deno, Bun, browsers, Cloudflare Workers).

**MCP Server** requires either:
- `stdio` transport → Node.js, Deno, Bun (any environment with stdin/stdout)
- `HTTP` transport → any environment that can host an HTTP server

### Browser Usage Example

```html
<!-- Use core schema in the browser for client-side validation -->
<script type="module">
  import { validate, createTask } from "https://esm.sh/@hippotask/core";

  const task = createTask({ title: "My task" });
  console.log(task); // Valid HippoTask object

  const result = validate({ title: "", status: "invalid" });
  if (!result.success) {
    console.error(result.error.issues);
    // → helpful validation errors
  }
</script>
```

### Deno Usage Example

```typescript
// deno.json — import map
{
  "imports": {
    "@hippotask/core": "npm:@hippotask/core",
    "@hippotask/adapter-linear": "npm:@hippotask/adapter-linear"
  }
}

// main.ts
import { createTask } from "@hippotask/core";
import { LinearAdapter } from "@hippotask/adapter-linear";

const adapter = new LinearAdapter();
await adapter.connect({ apiKey: Deno.env.get("LINEAR_API_KEY")! });
const tasks = await adapter.listTasks();
```

---

## 4. MCP Server Distribution

The MCP server has unique distribution needs because it's consumed by AI hosts, not just imported as a library.

### Distribution Methods

| Method | Command | Best For |
|--------|---------|----------|
| **npx (zero install)** | `npx @hippotask/mcp-server` | Quick testing, Claude Desktop config |
| **Global install** | `npm i -g @hippotask/mcp-server` | Permanent local setup |
| **Project dependency** | `npm i @hippotask/mcp-server` | Embedded in your app |
| **Docker** (future) | `docker run hippotask/mcp-server` | Isolated environments, CI |

### Claude Desktop Configuration

```jsonc
// ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "hippotask": {
      "command": "npx",
      "args": ["@hippotask/mcp-server"],
      "env": {
        "HIPPOTASK_STORE": "file",
        "HIPPOTASK_STORE_PATH": "~/.hippotask/tasks.json"
      }
    }
  }
}
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HIPPOTASK_STORE` | `memory` | Storage backend: `memory` or `file` |
| `HIPPOTASK_STORE_PATH` | `./hippotask-store.json` | File path for file store |
| `HIPPOTASK_TRANSPORT` | `stdio` | Transport: `stdio` or `http` |
| `HIPPOTASK_PORT` | `3000` | HTTP port (when transport=http) |
| `HIPPOTASK_LOG_LEVEL` | `info` | Logging: `debug`, `info`, `warn`, `error` |

### Programmatic Usage

```typescript
import { createServer } from "@hippotask/mcp-server";
import { JiraAdapter } from "@hippotask/adapter-jira";

const server = createServer({
  store: "file",
  storePath: "./my-tasks.json",
  adapters: [
    {
      platform: "jira",
      config: {
        domain: "myteam.atlassian.net",
        token: process.env.JIRA_TOKEN,
      },
    },
  ],
});

await server.start(); // Starts on stdio by default
```

---

## 5. JSON Schema Distribution

For non-TypeScript consumers, the JSON Schema is the primary distribution artifact.

### Access Methods

```bash
# Via npm package
npm install @hippotask/core
# → node_modules/@hippotask/core/schema/hippo-task.schema.json

# Via CDN (no install)
curl https://unpkg.com/@hippotask/core/schema/hippo-task.schema.json
curl https://cdn.jsdelivr.net/npm/@hippotask/core/schema/hippo-task.schema.json

# Via GitHub raw
curl https://raw.githubusercontent.com/tyherox/hippo-task/main/packages/core/schema/hippo-task.schema.json
```

### Usage in Other Languages

**Python:**
```python
import json
import jsonschema

with open("hippo-task.schema.json") as f:
    schema = json.load(f)

task = {"id": "...", "title": "My task", "status": "todo", ...}
jsonschema.validate(task, schema)  # Raises on invalid
```

**Go:**
```go
import "github.com/xeipuuv/gojsonschema"

schemaLoader := gojsonschema.NewReferenceLoader("file:///path/to/hippo-task.schema.json")
documentLoader := gojsonschema.NewStringLoader(taskJSON)
result, _ := gojsonschema.Validate(schemaLoader, documentLoader)
```

**Ruby, PHP, Java, C#, Rust:** — All have JSON Schema validation libraries. The schema file is the universal distribution artifact.

---

## 6. Documentation Site

### Hosting

| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| **GitHub Pages** | Free, auto-deploy from repo, custom domain | Static only | ✅ Day one |
| **Vercel** | Fast, preview deploys, edge | Requires account setup | ⏳ Future upgrade |
| **Starlight (Astro)** | Beautiful docs, search built-in, MDX | Build step | ✅ Docs framework |

**Decision:** Use **Starlight** (Astro-based docs framework) deployed to **GitHub Pages**. It's purpose-built for open source documentation, has built-in search, versioning, and looks professional.

### Site Structure

```
hippotask.dev (or GitHub Pages URL)
├── /                         → Landing page + quick start
├── /docs/
│   ├── getting-started/      → Installation, first task, 5-min tutorial
│   ├── schema/               → Schema reference (auto-generated from types)
│   ├── adapters/
│   │   ├── overview/         → Adapter comparison, provider scorecard
│   │   ├── jira/             → Jira adapter docs
│   │   ├── linear/           → Linear adapter docs
│   │   └── .../
│   ├── mcp/                  → MCP server setup, tools reference, agent examples
│   ├── guides/
│   │   ├── custom-adapter/   → Build your own adapter
│   │   ├── sync-patterns/    → Pull, push, bidirectional sync
│   │   └── ai-agents/        → AI agent integration patterns
│   └── engineering/          → Contributing, SOLID, TDD, code standards
├── /examples/                → Interactive examples / playground
├── /scorecard/               → Provider openness scorecard (interactive)
└── /api/                     → Auto-generated API reference (TypeDoc)
```

---

## 7. Release Process

### Workflow

```
Developer
  │
  ├── Creates feature branch
  ├── Writes code + tests (TDD)
  ├── Adds changeset: `pnpm changeset`
  │     → Answers: which packages changed? Major/minor/patch? Summary?
  │     → Creates .changeset/*.md file
  ├── Opens PR
  │
  ▼
CI Pipeline (GitHub Actions)
  │
  ├── Lint (Biome)
  ├── Type check (tsc --noEmit)
  ├── Unit tests (Vitest)
  ├── Integration tests (Vitest + msw)
  ├── Contract tests (all adapters)
  ├── Build (tsup)
  ├── Schema generation (zod-to-json-schema)
  ├── Bundle size check (size-limit)
  │
  ▼
Merge to main
  │
  ▼
Release PR (Changesets Action)
  │
  ├── Auto-created "Version Packages" PR
  ├── Aggregates all changesets
  ├── Bumps versions in package.json files
  ├── Updates CHANGELOG.md files
  │
  ▼
Merge Release PR
  │
  ▼
Publish (GitHub Actions)
  │
  ├── npm publish (all changed packages)
  ├── Git tag (per package)
  ├── GitHub Release (with changelog)
  ├── Docs site deploy
  └── JSON Schema files updated on CDN
```

### CI Pipeline Details

```yaml
# .github/workflows/ci.yml (conceptual)
name: CI
on: [pull_request]
jobs:
  quality:
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
      - run: pnpm install --frozen-lockfile
      - run: pnpm biome check .                    # Lint + format
      - run: pnpm turbo typecheck                  # tsc --noEmit
      - run: pnpm turbo test                       # Vitest (all packages)
      - run: pnpm turbo build                      # tsup
      - run: pnpm turbo schema:generate            # JSON Schema
      - run: pnpm size-limit                       # Bundle size budget
```

---

## 8. Versioning Strategy

### Independent Versioning

Each package is versioned independently. `@hippotask/core` v1.2.0 doesn't mean `@hippotask/adapter-jira` is also v1.2.0.

### Compatibility Matrix

The peer dependency ranges ensure compatibility:

```jsonc
// @hippotask/adapter-jira/package.json
{
  "peerDependencies": {
    "@hippotask/core": "^1.0.0"       // Any 1.x
  },
  "dependencies": {
    "@hippotask/adapter-common": "^1.0.0"
  }
}
```

### Semver Rules

| Change | Version Bump | Example |
|--------|-------------|---------|
| New optional field in schema | Minor | `1.0.0` → `1.1.0` |
| New enum value (e.g., new status) | Minor | `1.0.0` → `1.1.0` |
| New adapter published | N/A (new package) | `1.0.0` (new) |
| Bug fix in field mapping | Patch | `1.0.0` → `1.0.1` |
| Required field removed | Major | `1.0.0` → `2.0.0` |
| Required field type changed | Major | `1.0.0` → `2.0.0` |
| Adapter interface method added | Minor (optional) / Major (required) | Depends |

### Pre-1.0 Policy

While packages are `0.x.y`, we follow:
- `0.x.0` → breaking changes allowed
- `0.x.y` → non-breaking fixes and additions

This gives us room to iterate fast before committing to a stable API.
