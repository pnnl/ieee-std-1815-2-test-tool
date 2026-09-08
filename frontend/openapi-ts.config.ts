import { defineConfig } from '@hey-api/openapi-ts'

// Codegen config for the Rust web_server's utoipa-emitted OpenAPI spec.

export default defineConfig({
  input: './openapi.json',
  output: {
    path: 'src/api/generated',
    // No postProcess: prettier isn't a project dep, and our eslint config
    // is React-app-tuned (would flag generated code). Generated output is
    // gitignored and regenerated from the codegen, never hand-edited, so
    // house formatting concerns don't apply.
  },
  plugins: [
    {
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
