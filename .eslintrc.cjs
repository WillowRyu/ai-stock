module.exports = {
  root: true,
  parser: "@typescript-eslint/parser",
  parserOptions: {
    ecmaVersion: 2022,
    sourceType: "module",
    ecmaFeatures: { jsx: true },
  },
  plugins: ["@typescript-eslint", "react-hooks"],
  extends: ["eslint:recommended", "plugin:@typescript-eslint/recommended"],
  rules: {
    "no-unused-vars": "off",
    "@typescript-eslint/no-unused-vars": [
      "warn",
      { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
    ],
    // TS handles undef checks; turning this off avoids false positives on
    // browser globals (window, document, ...) that aren't worth enumerating.
    "no-undef": "off",
    // These are demoted to warnings so the starter config runs clean on the
    // existing codebase; treat them as backlog items, not blockers.
    "@typescript-eslint/no-explicit-any": "warn",
    "@typescript-eslint/no-var-requires": "warn",
    // Loaded just so existing `// eslint-disable-next-line react-hooks/exhaustive-deps`
    // directives in the codebase are recognized. Demote findings to warnings.
    "react-hooks/rules-of-hooks": "warn",
    "react-hooks/exhaustive-deps": "warn",
  },
  ignorePatterns: ["dist", "node_modules", "target", "app/gen", "*.cjs"],
  env: { browser: true, es2022: true, node: true },
};
