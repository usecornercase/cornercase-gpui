import { defineConfig } from 'astro/config';

export default defineConfig({
  site: process.env.SITE_ORIGIN || 'https://gui.usecornercase.dev',
  build: { inlineStylesheets: 'always', assetsPrefix: '.' },
  devToolbar: { enabled: false },
  vite: {
    build: { assetsInlineLimit: 0 },
    define: { 'import.meta.env.SITE_BASE': JSON.stringify(process.env.SITE_BASE ?? '') },
    server: { fs: { allow: ['..'] } },
  },
});
