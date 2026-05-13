import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./widget.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        // Semantic surfaces: callers use `bg-surface` etc. without remembering shades.
        // Hex with /opacity in className gives us tailwind's bg-surface/60 et al.
      },
      backdropBlur: { xs: "2px" },
    },
  },
  plugins: [],
} satisfies Config;
