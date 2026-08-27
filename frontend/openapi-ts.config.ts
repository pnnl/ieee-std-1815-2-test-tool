import { defineConfig } from '@hey-api/openapi-ts'

// Codegen config for the Rust web_server's utoipa-emitted OpenAPI spec.
//
// The backend must be running and reachable at OPENAPI_INPUT_URL before
// running `npm run generate:api`. The default points at the codegen-only
// dev port (8765) used during the migration; override OPENAPI_INPUT_URL
// to retarget (e.g. http://localhost:8001/openapi.json for the standard
// host-mapped Poem dev port, or http://mesa-poem-dev:8000/openapi.json
// from inside the compose network).
//
// Generated output is NOT committed (per #320): `src/api/generated/` is
// gitignored and regenerated from the backend OpenAPI spec at build/dev
// time via the Vite heyApiPlugin, or on demand via `npm run generate:api`.
// The generator is the source-of-truth; never hand-edit the contents.
const inputUrl =
  process.env.OPENAPI_INPUT_URL ?? 'http://localhost:8765/openapi.json'

export default defineConfig({
  input: inputUrl,
  output: {
    path: 'src/api/generated',
    // No postProcess: prettier isn't a project dep, and our eslint config
    // is React-app-tuned (would flag generated code). Generated output is
    // gitignored and regenerated from the codegen, never hand-edited, so
    // house formatting concerns don't apply.
  },
  plugins: [
    {
      // Fetch-based runtime client. Bundled into @hey-api/openapi-ts since
      // v0.73; no separate @hey-api/client-fetch dep needed.
      name: '@hey-api/client-fetch',
      // Override the spec's servers[0].url so the generated client.gen.ts
      // defaults to same-origin ('') instead of whatever localhost:PORT was
      // live at codegen time. All requests go through the Vite /api proxy in
      // dev, no hardcoded host:port in the
      // committed file. Empty string is correct here: the runtime concatenates
      // (baseUrl ?? '') + path, and path already begins with '/', so '' yields
      // the right relative URL while '/' would produce a double slash.
      baseUrl: '',
    },
    // TypeScript types for every component schema in the spec.
    '@hey-api/typescript',
    // Per-operation SDK functions (one function per operationId).
    '@hey-api/sdk',
  ],
})
