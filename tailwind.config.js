/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        hc: {
          bg: '#0A0A0B',
          surface: '#141416',
          line: '#2C2D33',
          strong: '#4A4A52',
          navy: '#1F2A44',
          'navy-text': '#AFC2EE',
          orange: '#FF7A1A',
          'orange-hover': '#FFA05A',
          cream: '#FFF0DC',
          body: '#E6D6BF',
          muted: '#B5A893',
          warn: '#FFD23F',
          up: '#7EE081',
          down: '#FF6B6B',
        },
      },
      fontFamily: {
        pixel: ['Silkscreen', 'monospace'],
        terminal: ['VT323', 'monospace'],
        body: ['"Pixelify Sans"', 'sans-serif'],
      },
      boxShadow: {
        'px-cream': '5px 5px 0 #FFF0DC',
        'px-orange': '5px 5px 0 #FF7A1A',
        'px-navy': '6px 6px 0 #1F2A44',
        'px-navy-lg': '12px 12px 0 #1F2A44',
      },
    },
  },
  plugins: [],
}
