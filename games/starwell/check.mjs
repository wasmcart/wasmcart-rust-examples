// Starwell's cart checks: played with pad input on a core WebGPU device and
// in compatibility mode. Each check has a control that must fail.
const MODE = { title: 0, play: 1, paused: 2, over: 3 };

/** Steer by the debug fields: toward the next ring, around crystals. */
function botInput(cart, h) {
  const px = cart.v('player.x'), py = cart.v('player.y');
  const gdz = cart.v('gate.dz');
  let tx = 0, ty = 0;
  if (gdz < 80) { tx = px + cart.v('gate.dx'); ty = py + cart.v('gate.dy'); }
  const pil = cart.v('pillars') || [];
  for (let k = 0; k < pil.length; k += 2) {
    const x = pil[k], dz = pil[k + 1];
    if (dz > 0 && dz < 16 && Math.abs(x - tx) < 2.1) tx = x + (tx >= x ? 2.3 : -2.3);
  }
  const by = cart.v('beam.y'), bdz = cart.v('beam.dz');
  if (bdz > 0 && bdz < 16 && Math.abs(by - ty) < 1.3) ty = by + (ty >= by ? 1.5 : -1.5);
  tx = Math.max(-6.3, Math.min(6.3, tx));
  ty = Math.max(-3.1, Math.min(3.1, ty));
  const sx = Math.max(-1, Math.min(1, (tx - px) * 0.9));
  const sy = Math.max(-1, Math.min(1, (ty - py) * 0.9));
  // Stick y is down-positive.
  return h.pad(0, { lx: sx, ly: -sy });
}

/** Run until `pred` or `max` frames, `pads` per frame. Returns frames run. */
async function runUntil(cart, max, pads, pred) {
  for (let i = 0; i < max; i += 10) {
    await cart.run(10, pads);
    if (pred()) return i + 10;
  }
  return max;
}

async function onDevice(ctx, featureLevel) {
  const h = ctx;
  const tag = `${ctx.name} [${featureLevel}]`;
  const shots = featureLevel === 'core' ? ctx.shots : null;
  const cart = await h.boot(ctx.file, { featureLevel });
  const A = h.pad(h.BTN.A);
  const START = h.pad(h.BTN.START);
  const idle2 = [h.idle(), h.idle()];

  // Title screen: lit, animated, and an idle pad never starts the game.
  await cart.run(90, idle2);
  const title = await cart.grab();
  h.savePng(shots, 'title.png', title);
  h.check(`${tag}: boots to the title`, cart.v('mode') === MODE.title, `mode ${cart.v('mode')}`);
  const shades = h.colours(title);
  h.check(`${tag}: title is a lit scene (many shades)`, shades > 120, `${shades} colours`);
  h.control(`${tag}: black frame as a lit scene`, h.colours(h.black(title)) > 120);
  await cart.run(600, idle2);
  h.check(`${tag}: ten idle seconds do not start a game`, cart.v('mode') === MODE.title, `mode ${cart.v('mode')}`);
  const title2 = await cart.grab();
  const titleMotion = h.changed(title, title2);
  h.check(`${tag}: the title scene moves`, titleMotion > 20000, `${titleMotion} px changed`);
  h.control(`${tag}: a frame compared with itself as motion`, h.changed(title2, title2) > 20000);

  // A starts; the second seat stays idle throughout.
  await cart.run(3, [A, h.idle()]);
  await cart.run(3, idle2);
  h.check(`${tag}: A on pad 1 starts a flight`, cart.v('mode') === MODE.play, `mode ${cart.v('mode')}`);

  // Steering: stick and d-pad both move the glider, either seat.
  const x0 = cart.v('player.x');
  await cart.run(25, [h.pad(0, { lx: 1 }), h.idle()]);
  const x1 = cart.v('player.x');
  h.check(`${tag}: left stick steers right`, x1 - x0 > 2, `x ${x0.toFixed(2)} -> ${x1.toFixed(2)}`);
  await cart.run(40, [h.pad(h.BTN.LEFT), h.idle()]);
  const x2 = cart.v('player.x');
  h.check(`${tag}: d-pad steers left`, x1 - x2 > 2, `x ${x1.toFixed(2)} -> ${x2.toFixed(2)}`);
  await cart.run(25, [h.idle(), h.pad(h.BTN.UP)]);
  const y1 = cart.v('player.y');
  h.check(`${tag}: a player on the second pad steers too`, y1 > 1, `y ${y1.toFixed(2)}`);
  await cart.run(25, idle2);
  const x3 = cart.v('player.x');
  h.control(`${tag}: no input as steering`, Math.abs(x3 - x2) > 2, `x ${x2.toFixed(2)} -> ${x3.toFixed(2)}`);

  // Pause on Start freezes the flight; Start resumes.
  await cart.run(2, [START, h.idle()]);
  await cart.run(2, idle2);
  const pd = cart.v('dist');
  await cart.run(30, idle2);
  h.check(`${tag}: Start pauses`, cart.v('mode') === MODE.paused && cart.v('dist') === pd, `mode ${cart.v('mode')}`);
  await cart.run(2, [START, h.idle()]);
  await cart.run(10, idle2);
  h.check(`${tag}: Start resumes`, cart.v('mode') === MODE.play && cart.v('dist') > pd);

  // Gameplay motion.
  const f1 = await cart.grab();
  await cart.run(6, (fr, c) => [botInput(c, h), h.idle()]);
  const f2 = await cart.grab();
  const moved = h.changed(f1, f2);
  h.check(`${tag}: flight moves between frames`, moved > 30000, `${moved} px changed`);
  h.savePng(shots, 'play-1.png', f2);

  // The bot flies sector 1 with the idle second pad connected.
  cart.audioRms();
  const frames = await runUntil(cart, 4200, (fr, c) => [botInput(c, h), h.idle()],
    () => cart.v('goal') === 1 || cart.v('mode') !== MODE.play);
  const goal = cart.v('goal');
  h.check(`${tag}: a bot steering by debug fields reaches sector 2`, goal === 1 && cart.v('sector') >= 2,
    `goal ${goal} sector ${cart.v('sector')} after ${frames} frames, rings ${cart.v('gates')}, hits ${cart.v('hits')}, energy ${cart.v('energy').toFixed(1)}`);
  const audio = cart.audioRms();
  const audible = (a) => a.rms > 0.01 && a.samples > 48000;
  h.check(`${tag}: audio reaches the ring`, audible(audio), `rms ${audio.rms.toFixed(3)} over ${audio.samples} samples`);
  h.control(`${tag}: the same length of silence as audio`, audible({ rms: 0, samples: audio.samples }));
  await cart.run(90, (fr, c) => [botInput(c, h), h.idle()]);
  const p2 = await cart.grab();
  h.savePng(shots, 'play-2.png', p2);
  h.check(`${tag}: sector 2 looks different (palette change)`, h.changed(f2, p2) > 100000, `${h.changed(f2, p2)} px changed`);
  h.check(`${tag}: score and best rise`, cart.v('score') > 1000 && cart.v('best') >= cart.v('score'), `score ${cart.v('score')} best ${cart.v('best')}`);
  cart.destroy();

  // Control: a player who never steers runs out of energy before sector 2,
  // which also exercises game over and restart.
  const c2 = await h.boot(ctx.file, { featureLevel });
  await c2.run(30, idle2);
  await c2.run(3, [A, h.idle()]);
  await runUntil(c2, 4200, idle2, () => c2.v('mode') === MODE.over || c2.v('goal') === 1);
  h.control(`${tag}: a hands-off flight as reaching the goal`, c2.v('goal') === 1, `sector ${c2.v('sector')} dist ${c2.v('dist').toFixed(0)} mode ${c2.v('mode')}`);
  h.check(`${tag}: running out of energy ends the flight`, c2.v('mode') === MODE.over, `mode ${c2.v('mode')}`);
  const overScore = c2.v('score');
  await c2.run(3, [A, h.idle()]);
  await c2.run(3, idle2);
  h.check(`${tag}: game over waits before accepting A`, c2.v('mode') === MODE.over);
  await c2.run(80, idle2);
  await c2.run(3, [A, h.idle()]);
  await c2.run(3, idle2);
  h.check(`${tag}: A restarts after game over`, c2.v('mode') === MODE.play && c2.v('score') < overScore && c2.v('best') >= overScore,
    `mode ${c2.v('mode')} score ${c2.v('score')} best ${c2.v('best')}`);
  const log = c2.log().filter((l) => /error|panic/i.test(l));
  h.check(`${tag}: no GPU errors or panics logged`, log.length === 0, log.slice(0, 2).join(' | '));
  c2.destroy();
}

export default async function check(ctx) {
  await onDevice(ctx, 'core');
  await onDevice(ctx, 'compatibility');
}
