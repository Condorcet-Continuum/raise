/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'software': '#0066CC',
        'system': '#009966',
        'hardware': '#CC6600',
        'ai': '#9933CC',
      },
    },
  },
  plugins: [],
}
