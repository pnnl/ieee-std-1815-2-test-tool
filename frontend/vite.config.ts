import { defineConfig, PluginOption } from 'vite'
import react from '@vitejs/plugin-react'
import { heyApiPlugin } from '@hey-api/vite-plugin'
import tailwindcss from '@tailwindcss/vite'
import fs from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'
import openApiConfig from './openapi-ts.config'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

// Dev-server proxy target. Server-side only; never bundled (no VITE_* prefix).
// In Docker, docker-compose.dev.yml sets BACKEND_HOST (mesa-backend-dev) and
// BACKEND_PORT=8000. Outside Docker, defaults target the Poem backend on its
// standard port.
const BACKEND_HOST = process.env.BACKEND_HOST || 'localhost'
const BACKEND_PORT = process.env.BACKEND_PORT || '8000'
const BACKEND_TARGET = `http://${BACKEND_HOST}:${BACKEND_PORT}`

// Static-data middleware. Serves /data/* file requests in dev (template
// profile, scenarios.json, etc.) — read-only, parent-dir-confined. The
// /api/profiles handlers that previously lived here have been retired
// (Phase 2 of #225); profile management now flows through the backend
// via the /api/profiles proxy entry below.
const dataServePlugin = (): PluginOption => ({
  name: 'serve-data',
  configureServer(server) {
    server.middlewares.use((req, res, next) => {
      // Handle /data file requests
      if (req.url?.startsWith('/data/')) {
        // Validate and normalize the requested path to prevent path traversal
        const requestedPath = req.url.substring(1) // Remove leading slash

        // Define allowed base directories
        const dockerBase = path.resolve('/data')
        const localBase = path.resolve(__dirname, '..', 'data')

        // Try Docker mount path first (/data is mounted in container)
        const dockerPath = path.resolve(dockerBase, requestedPath.substring(5)) // Remove 'data/' prefix
        // Then try local development path (../data relative to frontend/)
        const localPath = path.resolve(localBase, requestedPath.substring(5))

        let filePath = null
        try {
          // Ensure the resolved path is within the allowed directory
          if (
            dockerPath.startsWith(dockerBase) &&
            fs.existsSync(dockerPath) &&
            fs.statSync(dockerPath).isFile()
          ) {
            filePath = dockerPath
          } else if (
            localPath.startsWith(localBase) &&
            fs.existsSync(localPath) &&
            fs.statSync(localPath).isFile()
          ) {
            filePath = localPath
          }

          if (filePath) {
            const content = fs.readFileSync(filePath)
            const ext = path.extname(filePath)
            const contentType =
              ext === '.json' ? 'application/json' : 'text/plain'
            res.setHeader('Content-Type', contentType)
            res.setHeader('Cache-Control', 'no-cache')
            res.end(content)
            return
          }
        } catch (error) {
          console.error('Error serving data file:', error)
        }
      }
      next()
    })
  },
})

const restartOnOpenApiChange = (): PluginOption => {
  const openapiPath = path.resolve(__dirname, 'openapi.json')
  let restarting = false

  return {
    name: 'restart-on-openapi-change',
    configureServer(server) {
      server.watcher.add(openapiPath)

      server.watcher.on('change', async (file) => {
        if (file !== openapiPath || restarting) return

        restarting = true
        console.log('OpenAPI spec changed. Restarting Vite server...')
        try {
          await server.restart()
        } finally {
          restarting = false
        }
      })
    },
  }
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    dataServePlugin(),
    heyApiPlugin({ config: await openApiConfig }),
    restartOnOpenApiChange(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 3000,
    host: true, // Listen on all addresses for Docker
    watch: {
      usePolling: true, // Enable polling for Docker volumes
    },
    fs: {
      // Allow serving files from parent directory and Docker mounts
      allow: ['..', '/data'],
    },
    proxy: {
      // Catch-all: forward every /api/* request to the Poem backend.
      // Same-origin is now the default (baseUrl:''), so every generated
      // route — /api/profiles, /api/scenarios, /api/jobs, /api/enums,
      // /api/health — must resolve through this proxy in dev.
      '/api': {
        target: BACKEND_TARGET,
        changeOrigin: true,
        // Set Accept: text/event-stream on SSE event-stream sub-routes
        // (/api/jobs/*/events) so the backend keeps the connection open.
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            if (req.url?.includes('/events')) {
              proxyReq.setHeader('Accept', 'text/event-stream')
            }
          })
        },
      },
    },
  },
})
