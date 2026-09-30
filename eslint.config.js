import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default tseslint.config(
  { ignores: ["dist", "src/i18n/**"] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ["**/*.{ts,tsx}"],
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }],
      // Enforce space after comment markers (// or /*)
      "spaced-comment": ["error", "always", {
        "markers": ["/"],           // Allow /// for doc comments
        "exceptions": ["-", "+", "*"], // Allow //--- separator lines
        "block": {
          "balanced": true          // Require space in /* comment */
        }
      }],
    },
  },
  {
    // The @ alias maps to src/, so root config files (vite.config.ts, ...)
    // can't use it and keep relative imports.
    files: ["src/**/*.{ts,tsx}"],
    rules: {
      // Disallow relative imports, enforce @ alias usage
      "no-restricted-imports": ["error", {
        patterns: [{
          group: ["../*", "./*"],
          message: "Use @ path alias instead of relative paths (e.g., @/types/tab instead of ../types/tab)"
        }]
      }],
    },
  }
);
