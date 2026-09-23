import { createSarmgReactViteConfig } from "@sarmg/web-toolchain/vite";
import { mergeConfig } from "vite";

export default mergeConfig(createSarmgReactViteConfig(), {
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:18104",
        // Keep Host aligned with the browser's Origin for login and CSRF checks.
        changeOrigin: false,
      },
    },
  },
});
