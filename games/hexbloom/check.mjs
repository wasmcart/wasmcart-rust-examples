// Hexbloom's checks, run by scripts/check-carts.mjs on the Node host.
import { FIELDS, decide } from './bot.mjs';

export default async function ({ file, shots, boot, check, control, pad, idle, px, near, changed, countWhere, black, savePng }) {
  const read = (cart) => Object.fromEntries(FIELDS.map((n) => [n, cart.v(n)]));
  const asPad = (p) => pad(p.buttons || 0, p);

  // Title screen: the name in its colour, and it moves (the floor blooms).
  const cart = await boot(file);
  await cart.run(120, [idle(), idle()]);
  const title = await cart.grab();
  savePng(shots, 'title.png', title);
  const mint = (f) => countWhere(f, (r, g, b) => g > 220 && b > 180 && r > 110 && r < 190, { x: 280, y: 100, w: 400, h: 90 });
  check('hexbloom: title shows the name', mint(title) > 2500, `${mint(title)} px`);
  await cart.run(90, [idle(), idle()]);
  const title2 = await cart.grab();
  check('hexbloom: title screen is alive', changed(title, title2) > 200, `${changed(title, title2)} px changed`);
  control('the name check on a black frame', mint(black(title)) > 2500);
  control('the motion check of a frame against itself', changed(title, title) > 200);

  // An idle pad in each seat for ten seconds: nothing starts.
  await cart.run(600, [idle(), idle()]);
  const idleMode = cart.v('mode');
  control('ten seconds of idle pads as a started game', idleMode !== 0, `mode ${idleMode}`);

  // The bot on pad 0 (an idle pad sits in seat 2): right stick fires, left
  // stick moves, and it reaches the first goal, the wave 4 thorn flower.
  let wave4 = 0, started = false, minX = 1e9, maxX = -1e9, shot1 = false, shot2 = false, firstBloomAt = -1, lastMoving = null;
  const play = async (frames, seat) => {
    let f = read(cart);
    for (let i = 0; i < frames; i++) {
      const p = asPad(decide(f, cart.frame));
      await cart.run(1, seat === 0 ? [p, idle()] : [idle(), p]);
      f = read(cart);
      if (f.mode === 1) {
        started = true;
        minX = Math.min(minX, f.px); maxX = Math.max(maxX, f.px);
        if (!shot1 && f.wave === 2 && f.enemies >= 6) { shot1 = true; savePng(shots, 'play-1.png', await cart.grab()); }
        if (f.wave === 4) wave4++;
        if (!shot2 && wave4 === 420 && f.enemies >= 1) { shot2 = true; savePng(shots, 'play-2.png', await cart.grab()); }
      }
      if (f.first_bloom === 1 && firstBloomAt < 0) { firstBloomAt = cart.frame; break; }
    }
    return f;
  };
  const f = await play(6000, 0);
  check('hexbloom: pad A starts the game', started);
  check('hexbloom: the left stick moves the gardener', maxX - minX > 150, `x travel ${(maxX - minX).toFixed(0)}`);
  check('hexbloom: the right stick fires and kills', f.score > 1000, `score ${f.score}`);
  check('hexbloom: the bot fells the first thorn flower (wave 4)', firstBloomAt > 0, `at frame ${firstBloomAt}, lives ${f.lives}`);
  const a = await cart.grab();
  await cart.run(10, [idle(), idle()]);
  const b = await cart.grab();
  check('hexbloom: play moves between frames', changed(a, b) > 300, `${changed(a, b)} px`);
  // Cleared cells bloom: tinted floor pixels across the arena.
  const tinted = (fr) => countWhere(fr, (r, g, bb) => Math.max(r, g, bb) - Math.min(r, g, bb) > 25 && Math.max(r, g, bb) > 30, { x: 24, y: 56, w: 912, h: 468 });
  check('hexbloom: the floor blooms where enemies fell', tinted(b) > 20000, `${tinted(b)} tinted px`);
  control('the bloom check on a black frame', tinted(black(b)) > 20000);
  const audio = cart.audioRms();
  check('hexbloom: sound reaches the audio ring', audio.rms > 0.01, `rms ${audio.rms.toFixed(4)}`);
  cart.destroy();

  // Any seat drives the one gardener: the same bot on pad 1 with pad 0 idle.
  const c2 = await boot(file);
  await c2.run(30, [idle(), idle()]);
  let g = read(c2), start2 = false, kills2 = 0;
  for (let i = 0; i < 900; i++) {
    const p = asPad(decide(g, c2.frame));
    await c2.run(1, [idle(), p]);
    g = read(c2);
    if (g.mode === 1) start2 = true;
  }
  kills2 = c2.v('kills');
  check('hexbloom: a player on pad 2 starts and plays', start2 && kills2 > 3, `kills ${kills2}`);
  const quiet = c2.audioRms();
  c2.destroy();

  // Control: the same seat with no input never plays.
  const c3 = await boot(file);
  await c3.run(2, [idle(), idle()]);
  const before = c3.audioRms();
  control('the first frames, before any sound has loaded, as sound', before.rms > 0.01, `rms ${before.rms.toFixed(4)}`);
  await c3.run(900, [idle(), idle()]);
  control('a run with no input as a played game', c3.v('mode') !== 0 || c3.v('kills') > 0, `mode ${c3.v('mode')}`);
  void quiet; void px; void near;
  c3.destroy();
}
