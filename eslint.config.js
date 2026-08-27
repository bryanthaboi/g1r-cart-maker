import js from "@eslint/js";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsparser from "@typescript-eslint/parser";
import hooks from "eslint-plugin-react-hooks";

export default [
  { ignores: ["dist", "src-tauri", "target", "node_modules"] },
  js.configs.recommended,
  {
    files: ["tools/**/*.mjs"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: { process: "readonly", console: "readonly", fetch: "readonly", URL: "readonly" },
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsparser,
      parserOptions: { ecmaVersion: 2022, sourceType: "module", ecmaFeatures: { jsx: true } },
      globals: { window: "readonly", document: "readonly", console: "readonly", navigator: "readonly", setTimeout: "readonly", clearTimeout: "readonly", requestAnimationFrame: "readonly", cancelAnimationFrame: "readonly", HTMLElement: "readonly", HTMLCanvasElement: "readonly", HTMLImageElement: "readonly", HTMLInputElement: "readonly", HTMLDivElement: "readonly", Image: "readonly", FileReader: "readonly", File: "readonly", Blob: "readonly", URL: "readonly", crypto: "readonly", performance: "readonly", KeyboardEvent: "readonly", MouseEvent: "readonly", DragEvent: "readonly", ClipboardEvent: "readonly", ResizeObserver: "readonly", localStorage: "readonly", fetch: "readonly", structuredClone: "readonly" },
    },
    plugins: { "@typescript-eslint": tseslint, "react-hooks": hooks },
    rules: {
      ...tseslint.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
      "no-undef": "off",
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
    },
  },
];
