import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./widget.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: { extend: {} },
  plugins: [],
} satisfies Config;
