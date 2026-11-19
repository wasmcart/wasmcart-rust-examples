#!/usr/bin/env node
// Make each game's launcher art (SPEC: manifest `art`) into games/<name>/art:
// icon, cover, background and logo drawn on a canvas in headless Chromium,
// plus the title screen and play screenshots the cart checks saved.
//
//   node scripts/check-carts.mjs --shots shots   # the screenshots first
//   node scripts/make-art.mjs [game ...]
//
// Everything is drawn from code and the games' own frames, so the art is
// original and CC0 like the rest of the assets.
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { chromium } from 'playwright';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');

/** Per game: name, tagline, colours, screenshot labels, and an icon painter (runs in the page). */
const GAMES = {
  hexbloom: {
    title: 'HEXBLOOM',
    tagline: 'Clear the overgrown garden',
    bg: '#070914', ink: '#96ffdc', glow: '#ff5ac8',
    shots: [['play-1.png', 'A swarm of seeds and darters closing in on the gardener'], ['play-2.png', 'The thorn flower spraying rings of thorns over a blooming floor']],
    icon: `
      ctx.fillStyle = '#070914'; ctx.fillRect(0, 0, S, S);
      const hex = (x, y, r, fill, stroke) => { ctx.beginPath(); for (let k = 0; k < 6; k++) { const a = Math.PI / 6 + k * Math.PI / 3; ctx.lineTo(x + r * Math.cos(a), y + r * Math.sin(a)); } ctx.closePath(); if (fill) { ctx.fillStyle = fill; ctx.fill(); } if (stroke) { ctx.strokeStyle = stroke; ctx.lineWidth = S * 0.012; ctx.stroke(); } };
      const cols = ['#78eb78', '#ff5ac8', '#ffa03c', '#5ac8ff', '#78eb78', '#ff5ac8'];
      for (let k = 0; k < 6; k++) { const a = k * Math.PI / 3; hex(S/2 + Math.cos(a) * S*0.2, S/2 + Math.sin(a) * S*0.2, S*0.105, cols[k] + '55', cols[k]); }
      ctx.shadowColor = '#96ffdc'; ctx.shadowBlur = S * 0.06;
      hex(S/2, S/2, S*0.11, '#ffd278', '#ffffff');
      ctx.shadowBlur = 0;
      ctx.fillStyle = '#96ffdc'; ctx.beginPath(); ctx.moveTo(S*0.5, S*0.43); ctx.lineTo(S*0.45, S*0.56); ctx.lineTo(S*0.55, S*0.56); ctx.fill();`,
  },
  tetherline: {
    title: 'TETHERLINE',
    tagline: 'Swing across five islands',
    bg: '#5c8fd6', ink: '#fff5dc', glow: '#ff6e5a',
    keyShot: 1, // the first shot carries a sign's text; key art has none
    shots: [['play-1.png', 'Swinging on a tether over the first gap of the meadow'], ['play-2.png', 'Crumbling ledges over a bed of spikes on the third island']],
    icon: `
      const g = ctx.createLinearGradient(0, 0, 0, S); g.addColorStop(0, '#5ca0e6'); g.addColorStop(1, '#ecd6aa'); ctx.fillStyle = g; ctx.fillRect(0, 0, S, S);
      ctx.fillStyle = '#6ec45a'; ctx.fillRect(0, S*0.8, S, S*0.2); ctx.fillStyle = '#7a5440'; ctx.fillRect(0, S*0.84, S, S*0.16);
      const ring = [S*0.62, S*0.24];
      ctx.strokeStyle = '#78dcff'; ctx.lineWidth = S*0.035; ctx.beginPath(); ctx.arc(ring[0], ring[1], S*0.07, 0, Math.PI*2); ctx.stroke();
      ctx.strokeStyle = '#ffffff'; ctx.lineWidth = S*0.012; ctx.stroke();
      const body = [S*0.38, S*0.58];
      ctx.strokeStyle = '#fae6be'; ctx.lineWidth = S*0.014; ctx.beginPath(); ctx.moveTo(body[0], body[1]); ctx.lineTo(ring[0], ring[1]); ctx.stroke();
      ctx.strokeStyle = '#f05a5a'; ctx.lineWidth = S*0.035; ctx.lineCap = 'round'; ctx.beginPath(); ctx.moveTo(body[0], body[1] - S*0.03); ctx.quadraticCurveTo(body[0] - S*0.12, body[1] - S*0.02, body[0] - S*0.2, body[1] + S*0.05); ctx.stroke();
      ctx.fillStyle = '#fad66b'; ctx.beginPath(); ctx.ellipse(body[0], body[1], S*0.1, S*0.115, 0, 0, Math.PI*2); ctx.fill();
      ctx.strokeStyle = '#73502a'; ctx.lineWidth = S*0.012; ctx.stroke();
      ctx.fillStyle = '#404d66'; ctx.beginPath(); ctx.arc(body[0] + S*0.04, body[1] - S*0.03, S*0.035, 0, Math.PI*2); ctx.fill();
      ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(body[0] + S*0.05, body[1] - S*0.04, S*0.013, 0, Math.PI*2); ctx.fill();`,
  },
  floe: {
    title: 'BUMPER FLOE',
    tagline: 'Last one on the ice wins',
    bg: '#143c5f', ink: '#ffffff', glow: '#e8504a',
    shots: [['play-1.png', 'Four bumpers on the floe as an edge cracks away'], ['play-2.png', 'Two players charging dashes on the ice'], ['match-end.png', 'The match winner crowned on the end screen']],
    icon: `
      ctx.fillStyle = '#143c5f'; ctx.fillRect(0, 0, S, S);
      ctx.fillStyle = '#6aa0c8'; ctx.beginPath(); ctx.ellipse(S/2, S*0.55, S*0.4, S*0.3, 0, 0, Math.PI*2); ctx.fill();
      ctx.fillStyle = '#d2eaf7'; ctx.beginPath(); ctx.ellipse(S/2, S*0.52, S*0.4, S*0.3, 0, 0, Math.PI*2); ctx.fill();
      const bump = (x, y, c, d) => { ctx.fillStyle = c; ctx.beginPath(); ctx.arc(x, y, S*0.12, 0, Math.PI*2); ctx.fill(); ctx.strokeStyle = 'rgba(0,0,0,0.35)'; ctx.lineWidth = S*0.012; ctx.stroke();
        ctx.fillStyle = 'rgba(255,255,255,0.45)'; ctx.beginPath(); ctx.arc(x - S*0.04, y - S*0.045, S*0.035, 0, Math.PI*2); ctx.fill();
        for (const e of [-1, 1]) { ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(x + d*S*0.03 + e*S*0.035, y + S*0.01, S*0.025, 0, Math.PI*2); ctx.fill(); ctx.fillStyle = '#111'; ctx.beginPath(); ctx.arc(x + d*S*0.04 + e*S*0.035, y + S*0.015, S*0.012, 0, Math.PI*2); ctx.fill(); } };
      bump(S*0.37, S*0.5, '#e8504a', 1); bump(S*0.64, S*0.52, '#4a8ae8', -1);
      ctx.fillStyle = '#fff'; for (const [x, y] of [[0.5, 0.42], [0.47, 0.6], [0.53, 0.38]]) { ctx.beginPath(); ctx.arc(S*x, S*y, S*0.014, 0, Math.PI*2); ctx.fill(); }`,
  },
  starwell: {
    title: 'STARWELL',
    tagline: 'Fly the crystal canyon',
    bg: '#0b1630', ink: '#d8f4ff', glow: '#ffd060',
    shots: [['play-1.png', 'The glider lining up a gold ring between crystal pillars'], ['play-2.png', 'Crossing into sector 2 as the canyon changes colour']],
    icon: `
      const g = ctx.createLinearGradient(0, 0, 0, S); g.addColorStop(0, '#0b1630'); g.addColorStop(1, '#1d3550'); ctx.fillStyle = g; ctx.fillRect(0, 0, S, S);
      ctx.strokeStyle = 'rgba(120,200,255,0.55)'; ctx.lineWidth = S*0.008;
      const vx = S/2, vy = S*0.42;
      for (let k = -6; k <= 6; k++) { ctx.beginPath(); ctx.moveTo(vx, vy); ctx.lineTo(vx + k * S*0.18, S); ctx.stroke(); }
      for (let k = 1; k < 7; k++) { const t = Math.pow(k / 7, 2); ctx.beginPath(); ctx.moveTo(0, vy + (S - vy) * t); ctx.lineTo(S, vy + (S - vy) * t); ctx.stroke(); }
      ctx.fillStyle = '#7ad8ff'; for (const [x, h] of [[0.3, 0.3], [0.72, 0.26]]) { ctx.beginPath(); ctx.moveTo(S*x, S*(0.75 - h)); ctx.lineTo(S*(x + 0.04), S*0.72); ctx.lineTo(S*(x - 0.04), S*0.72); ctx.fill(); }
      ctx.shadowColor = '#ffd060'; ctx.shadowBlur = S*0.06; ctx.strokeStyle = '#ffd060'; ctx.lineWidth = S*0.045;
      ctx.beginPath(); ctx.arc(S/2, S*0.45, S*0.16, 0, Math.PI*2); ctx.stroke(); ctx.shadowBlur = 0;
      ctx.fillStyle = '#eef6ff'; ctx.beginPath(); ctx.moveTo(S*0.5, S*0.68); ctx.lineTo(S*0.3, S*0.8); ctx.lineTo(S*0.7, S*0.8); ctx.fill();
      for (const x of [0.44, 0.56]) { ctx.fillStyle = 'rgba(200,240,255,0.5)'; ctx.fillRect(S*x - S*0.01, S*0.8, S*0.02, S*0.15); }`,
  },
  hillfold: {
    title: 'HILLFOLD',
    tagline: 'Herd the flock home before sundown',
    bg: '#2a3a1c', ink: '#fff6dc', glow: '#ff9a40',
    keyShot: 1, // the first shot carries a hint; key art has none
    shots: [['play-1.png', 'The dog driving the flock across Home Meadow'], ['play-2.png', 'Stone walls to steer round on the second field'], ['cleared.png', 'The fold full and the gate shut as the sun gets low']],
    icon: `
      const g = ctx.createLinearGradient(0, 0, 0, S); g.addColorStop(0, '#f4b066'); g.addColorStop(0.45, '#f7d79a'); g.addColorStop(0.46, '#5e9a3c'); g.addColorStop(1, '#3c6e28'); ctx.fillStyle = g; ctx.fillRect(0, 0, S, S);
      ctx.fillStyle = '#ffe6a0'; ctx.beginPath(); ctx.arc(S*0.74, S*0.36, S*0.09, 0, Math.PI*2); ctx.fill();
      ctx.fillStyle = '#4f8a33'; ctx.beginPath(); ctx.ellipse(S*0.3, S*0.52, S*0.45, S*0.12, 0, Math.PI, 0); ctx.fill();
      ctx.strokeStyle = '#7a5232'; ctx.lineWidth = S*0.018;
      for (const y of [0.66, 0.74]) { ctx.beginPath(); ctx.moveTo(S*0.52, S*y); ctx.lineTo(S*0.92, S*y); ctx.stroke(); }
      for (let k = 0; k < 5; k++) { ctx.beginPath(); ctx.moveTo(S*(0.54 + k*0.09), S*0.62); ctx.lineTo(S*(0.54 + k*0.09), S*0.8); ctx.stroke(); }
      const sheep = (x, y, s) => { ctx.fillStyle = '#f4f1ea'; for (const [dx, dy, r] of [[0, 0, 0.07], [-0.05, 0.01, 0.05], [0.05, 0.01, 0.05], [0, -0.04, 0.05]]) { ctx.beginPath(); ctx.arc(S*(x + dx*s), S*(y + dy*s), S*r*s, 0, Math.PI*2); ctx.fill(); }
        ctx.fillStyle = '#2a2420'; ctx.beginPath(); ctx.ellipse(S*(x + 0.08*s), S*(y - 0.01*s), S*0.035*s, S*0.03*s, 0, 0, Math.PI*2); ctx.fill();
        ctx.fillRect(S*(x - 0.04*s), S*(y + 0.05*s), S*0.015*s, S*0.05*s); ctx.fillRect(S*(x + 0.03*s), S*(y + 0.05*s), S*0.015*s, S*0.05*s); };
      sheep(0.2, 0.76, 1.2); sheep(0.38, 0.84, 1.0); sheep(0.3, 0.62, 0.8);
      ctx.fillStyle = '#231a14'; ctx.beginPath(); ctx.ellipse(S*0.1, S*0.9, S*0.06, S*0.035, 0, 0, Math.PI*2); ctx.fill();
      ctx.beginPath(); ctx.arc(S*0.15, S*0.86, S*0.03, 0, Math.PI*2); ctx.fill();`,
  },
  'lantern-links': {
    title: 'LANTERN LINKS',
    tagline: 'Night-garden minigolf',
    bg: '#06100e', ink: '#ffe0a0', glow: '#ffae4a',
    keyShot: 1,
    shots: [['play-1.png', 'Lining up the first putt between the lanterns'], ['play-2.png', 'Timing a putt past the spinning turnstile'], ['card.png', 'The scorecard at the end of the round']],
    icon: `
      ctx.fillStyle = '#06100e'; ctx.fillRect(0, 0, S, S);
      const glow = (x, y, r) => { const g = ctx.createRadialGradient(x, y, 0, x, y, r); g.addColorStop(0, 'rgba(255,190,100,0.55)'); g.addColorStop(1, 'rgba(255,190,100,0)'); ctx.fillStyle = g; ctx.fillRect(x - r, y - r, r*2, r*2); };
      ctx.fillStyle = '#8a7462'; ctx.fillRect(S*0.08, S*0.2, S*0.84, S*0.66);
      const g2 = ctx.createLinearGradient(0, S*0.26, 0, S*0.8); g2.addColorStop(0, '#2f6a35'); g2.addColorStop(1, '#3d7d40'); ctx.fillStyle = g2; ctx.fillRect(S*0.14, S*0.26, S*0.72, S*0.54);
      glow(S*0.12, S*0.18, S*0.3); glow(S*0.88, S*0.18, S*0.3);
      ctx.fillStyle = '#ffe2a8'; for (const x of [0.12, 0.88]) { ctx.beginPath(); ctx.arc(S*x, S*0.16, S*0.035, 0, Math.PI*2); ctx.fill(); }
      ctx.fillStyle = '#0a0a08'; ctx.beginPath(); ctx.arc(S*0.68, S*0.56, S*0.06, 0, Math.PI*2); ctx.fill();
      ctx.strokeStyle = '#3a2a1a'; ctx.lineWidth = S*0.014; ctx.beginPath(); ctx.moveTo(S*0.68, S*0.56); ctx.lineTo(S*0.68, S*0.32); ctx.stroke();
      ctx.fillStyle = '#ffb648'; ctx.beginPath(); ctx.moveTo(S*0.68, S*0.32); ctx.lineTo(S*0.8, S*0.36); ctx.lineTo(S*0.68, S*0.4); ctx.fill();
      ctx.fillStyle = 'rgba(0,0,0,0.4)'; ctx.beginPath(); ctx.arc(S*0.33, S*0.64, S*0.05, 0, Math.PI*2); ctx.fill();
      ctx.fillStyle = '#fafaf4'; ctx.beginPath(); ctx.arc(S*0.31, S*0.62, S*0.05, 0, Math.PI*2); ctx.fill();
      ctx.fillStyle = 'rgba(255,240,190,0.85)'; for (let k = 1; k < 5; k++) { ctx.beginPath(); ctx.arc(S*(0.31 + k*0.07), S*(0.62 - k*0.012), S*0.012, 0, Math.PI*2); ctx.fill(); }`,
  },
};

const PAGE =`<!doctype html><html><body style="margin:0;background:transparent"><canvas id=c></canvas></body></html>`;

async function render(page, w, h, body, type = 'png') {
  const url = await page.evaluate(async ({ w, h, body, type }) => {
    const c = document.getElementById('c');
    c.width = w; c.height = h;
    const ctx = c.getContext('2d');
    const load = (src) => new Promise((ok, no) => { const i = new Image(); i.onload = () => ok(i); i.onerror = no; i.src = src; });
    await new Function('ctx', 'W', 'H', 'load', `return (async () => { ${body} })()`)(ctx, w, h, load);
    return c.toDataURL(type === 'jpeg' ? 'image/jpeg' : 'image/png', 0.9);
  }, { w, h, body, type });
  return Buffer.from(url.split(',')[1], 'base64');
}

const dataUrl = (file) => 'data:image/png;base64,' + readFileSync(file).toString('base64');

const games = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(GAMES);
const browser = await chromium.launch();
const page = await browser.newPage();
await page.setContent(PAGE);
const FONT = `'DejaVu Sans', 'Verdana', sans-serif`;

for (const name of games) {
  const g = GAMES[name];
  const shots = path.join(ROOT, 'shots', name);
  const out = path.join(ROOT, 'games', name, 'art');
  for (const f of ['title.png', ...g.shots.map((s) => s[0])]) {
    if (!existsSync(path.join(shots, f))) throw new Error(`${name}: ${f} missing in ${shots}; run scripts/check-carts.mjs --shots shots first`);
  }
  mkdirSync(out, { recursive: true });
  const play = JSON.stringify(dataUrl(path.join(shots, g.shots[g.keyShot ?? 0][0])));
  const vars = `const C = ${JSON.stringify(g)}; const FONT = ${JSON.stringify(FONT)};`;
  // Titles are drawn with a soft glow and a dark edge so they read on any art.
  const titleText = `
    function title(text, x, y, size, maxW) {
      ctx.font = '900 ' + size + 'px ' + FONT; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      let s = size; while (ctx.measureText(text).width > maxW) { s -= 4; ctx.font = '900 ' + s + 'px ' + FONT; }
      ctx.lineJoin = 'round'; ctx.shadowColor = C.glow; ctx.shadowBlur = s * 0.35;
      ctx.lineWidth = s * 0.16; ctx.strokeStyle = 'rgba(5,8,20,0.85)'; ctx.strokeText(text, x, y);
      ctx.shadowBlur = 0; ctx.fillStyle = C.ink; ctx.fillText(text, x, y);
    }`;
  // A frame scaled to cover a w x h box (cropping the HUD rows at the top).
  const coverImg = `
    async function coverImg(src, w, h, blur) {
      const img = await load(src);
      const sx = 40, sy = 60, sw = img.width - 80, sh = img.height - 110;
      const k = Math.max(w / sw, h / sh);
      const dw = sw * k, dh = sh * k;
      ctx.filter = blur ? 'blur(' + blur + 'px)' : 'none';
      ctx.drawImage(img, sx, sy, sw, sh, (w - dw) / 2, (h - dh) / 2, dw, dh);
      ctx.filter = 'none';
    }`;

  writeFileSync(path.join(out, 'icon.png'), await render(page, 512, 512, `${vars} const S = 512; ${g.icon}`));

  writeFileSync(path.join(out, 'background.jpg'), await render(page, 1920, 1080, `${vars} ${coverImg}
    await coverImg(${play}, W, H, 3);
    const v = ctx.createRadialGradient(W/2, H/2, H*0.3, W/2, H/2, H*0.95);
    v.addColorStop(0, 'rgba(0,0,0,0)'); v.addColorStop(1, 'rgba(0,0,0,0.55)');
    ctx.fillStyle = v; ctx.fillRect(0, 0, W, H);`, 'jpeg'));

  writeFileSync(path.join(out, 'cover.jpg'), await render(page, 800, 1200, `${vars} ${coverImg} ${titleText}
    ctx.fillStyle = C.bg; ctx.fillRect(0, 0, W, H);
    await coverImg(${play}, W, H * 0.72, 0);
    const fade = ctx.createLinearGradient(0, H * 0.45, 0, H * 0.75);
    fade.addColorStop(0, 'rgba(0,0,0,0)'); fade.addColorStop(1, C.bg);
    ctx.fillStyle = fade; ctx.fillRect(0, H * 0.45, W, H * 0.3);
    ctx.fillStyle = C.bg; ctx.fillRect(0, H * 0.75, W, H * 0.25);
    title(C.title, W / 2, H * 0.8, 120, W * 0.88);
    ctx.font = '600 40px ' + FONT; ctx.fillStyle = C.ink; ctx.globalAlpha = 0.85; ctx.fillText(C.tagline, W / 2, H * 0.89);
    ctx.globalAlpha = 0.6; ctx.font = '500 26px ' + FONT; ctx.fillText('a wasmcart game', W / 2, H * 0.955);`, 'jpeg'));

  writeFileSync(path.join(out, 'logo.png'), await render(page, 1280, 360, `${vars} ${titleText}
    title(C.title, W / 2, H / 2, 200, W * 0.92);`));

  copyFileSync(path.join(shots, 'title.png'), path.join(out, 'title.png'));
  const screenshots = g.shots.map(([f, label], i) => {
    const dest = `screenshot-${i + 1}.png`;
    copyFileSync(path.join(shots, f), path.join(out, dest));
    return { path: dest, label };
  });
  writeFileSync(path.join(out, 'art.json'), JSON.stringify({
    icon: 'icon.png', cover: 'cover.jpg', background: 'background.jpg', logo: 'logo.png', title: 'title.png', screenshots,
  }, null, 2) + '\n');
  console.log(`${name}: art written to ${path.relative(ROOT, out)}`);
}
await browser.close();
