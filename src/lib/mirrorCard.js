// Shareable Mirror Mode card (1200 × 675, the X / Telegram preview ratio), drawn on a canvas in the browser.
const W = 1200;
const H = 675;
const C = {
  bg: '#0A0A0B',
  surface: '#141416',
  line: '#2C2D33',
  orange: '#FF7A1A',
  cream: '#FFF0DC',
  body: '#E6D6BF',
  muted: '#B5A893',
  down: '#FF6B6B',
  up: '#7EE081',
  navy: '#1F2A44',
};

function loadImage(src) {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

async function fontsReady() {
  if (!document.fonts?.load) return;
  await Promise.all(['700 40px Silkscreen', '40px Silkscreen', '200px VT323', '28px "Pixelify Sans"'].map((f) => document.fonts.load(f).catch(() => {})));
}

function text(ctx, value, x, y, { font, color = C.cream, align = 'left' }) {
  ctx.font = font;
  ctx.fillStyle = color;
  ctx.textAlign = align;
  ctx.fillText(value, x, y);
  return ctx.measureText(value).width;
}

// Draws the card and returns the canvas.
export async function drawMirrorCard(result, { canvas = document.createElement('canvas'), mascotSrc = '/herdcall-hero.webp' } = {}) {
  await fontsReady();
  const mascot = await loadImage(mascotSrc);
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d');
  ctx.imageSmoothingEnabled = false;
  ctx.textBaseline = 'alphabetic';

  // Frame: pixel border with an offset shadow, like the site's cards.
  ctx.fillStyle = C.bg;
  ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = C.navy;
  ctx.fillRect(40, 40, W - 64, H - 64);
  ctx.fillStyle = C.surface;
  ctx.fillRect(24, 24, W - 64, H - 64);
  ctx.strokeStyle = C.orange;
  ctx.lineWidth = 6;
  ctx.strokeRect(27, 27, W - 70, H - 70);

  const left = 64;
  const name = result.ticker ? `$${result.ticker}` : 'this token';
  text(ctx, 'HERDCALL · MIRROR MODE', left, 96, { font: '700 26px Silkscreen', color: C.orange });
  if (result.sample) {
    const label = 'SAMPLE DATA · NOT A REAL TOKEN';
    ctx.font = '700 18px Silkscreen';
    const w = ctx.measureText(label).width + 24;
    ctx.fillStyle = C.orange;
    ctx.fillRect(W - 76 - w, 70, w, 36);
    text(ctx, label, W - 76 - w + 12, 95, { font: '700 18px Silkscreen', color: C.bg });
  }

  // Headline number.
  const count = String(result.walletsBefore);
  const countWidth = text(ctx, count, left - 6, 330, { font: '230px VT323', color: C.orange });
  const headX = left + countWidth + 24;
  const lines = result.walletsBefore === 0 ? ['NOBODY GOT IN', 'BEFORE YOU'] : [result.walletsBefore === 1 ? 'WALLET GOT IN' : 'WALLETS GOT IN', `BEFORE YOU${result.partial ? ' (AT LEAST)' : ''}`];
  text(ctx, lines[0], headX, 232, { font: '700 40px Silkscreen' });
  text(ctx, lines[1], headX, 284, { font: '700 40px Silkscreen' });
  text(ctx, `on ${name}`, headX, 326, { font: '30px "Pixelify Sans"', color: C.muted });

  // Stats row.
  const stats = [
    ['YOU PAID', result.priceMultiple ? `${result.priceMultiple.toFixed(2)}x` : '—', result.priceMultiple > 1 ? C.down : C.cream],
    ['FIRST BLOCK', String(result.firstBlockWallets), C.cream],
    ['SOL BEFORE YOU', result.beforeSol.toFixed(2), C.cream],
  ];
  const boxW = 230;
  stats.forEach(([label, value, color], i) => {
    const x = left + i * (boxW + 18);
    ctx.fillStyle = C.bg;
    ctx.fillRect(x, 360, boxW, 108);
    ctx.strokeStyle = C.line;
    ctx.lineWidth = 4;
    ctx.strokeRect(x + 2, 362, boxW - 4, 104);
    text(ctx, label, x + 16, 392, { font: '16px Silkscreen', color: C.muted });
    text(ctx, value, x + 16, 450, { font: '58px VT323', color });
  });

  // The HerdCall version.
  if (result.herd) {
    ctx.fillStyle = C.bg;
    ctx.fillRect(left, 486, 726, 92);
    ctx.strokeStyle = C.orange;
    ctx.strokeRect(left + 2, 488, 722, 88);
    text(ctx, 'ON HERDCALL: ONE PRICE FOR EVERYONE', left + 18, 518, { font: '700 18px Silkscreen', color: C.orange });
    const gain = result.herd.shareGain;
    const gainText = gain == null ? '' : ` · ${gain >= 0 ? '+' : ''}${(gain * 100).toFixed(0)}% bigger share for you`;
    text(ctx, `est. ${result.herd.mcapSol.toFixed(1)} SOL mcap${gainText}`, left + 18, 562, { font: '40px VT323', color: C.cream });
  }

  // Mascot.
  if (mascot) {
    const size = 300;
    const x = W - 76 - size;
    const y = 150;
    ctx.fillStyle = C.bg;
    ctx.fillRect(x - 6, y - 6, size + 12, size + 12);
    const side = Math.min(mascot.width, mascot.height);
    ctx.drawImage(mascot, (mascot.width - side) / 2, (mascot.height - side) / 2, side, side, x, y, size, size);
    ctx.strokeStyle = C.orange;
    ctx.lineWidth = 6;
    ctx.strokeRect(x - 3, y - 3, size + 6, size + 6);
    text(ctx, 'NO SNIPERS.', x, y + size + 48, { font: '700 22px Silkscreen' });
    text(ctx, 'JUST THE HERD.', x, y + size + 80, { font: '700 22px Silkscreen', color: C.orange });
  }

  text(ctx, 'herdcall.fun/mirror · estimate from public chain data · not financial advice', left, H - 56, { font: '20px "Pixelify Sans"', color: C.muted });
  return canvas;
}

export function cardFileName(result) {
  return `herdcall-mirror-${(result.ticker || 'token').toLowerCase()}.png`;
}
