import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig(({ command }) => ({
  plugins: [react()],
  // Use absolute base during dev and relative base for production builds (required for Tauri)
  base: command === 'build' ? './' : '/',
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true
  },
  preview: {
    host: '127.0.0.1',
    port: 5173
  },
  build: {
    outDir: 'dist'
  }
}))
