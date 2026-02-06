import { defineConfig } from 'vite';

export default defineConfig({
        root: './',
        publicDir: './public',
        test: {
                environment: 'happy-dom',
                setupFiles: './src/test/setup.ts',
                globals: true,
        },
        build: {
                outDir: '../static',
                emptyOutDir: true,
                manifest: true, // ✅ WAJIB agar static/.vite/manifest.json dibuat
                rollupOptions: {
                        input: './index.html',
                },
        },
        server: {
                port: 5173,
                strictPort: true,
        },
});
