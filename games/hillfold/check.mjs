// Hillfold's cart checks: played with pad input on a core WebGPU device and
// in compatibility mode. Each check has a control that must fail.
const MODE = { title: 0, intro: 1, play: 2, paused: 3, cleared: 4, over: 5, victory: 6 };

/** Run until `pred` or `max` frames, `pads` per frame. Returns frames run. */
async function runUntil(cart, max, pads, pred) {
  for (let i = 0; i < max; i += 30) {
    await cart.run(30, pads);
    if (pred()) return i + 30;
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

  // The first frames are silent (the music fades in): the audio control.
  await cart.run(3, idle2);
  const early = cart.audioRms();
  // Title: a lit, moving scene; an idle pad never starts the game.
  await cart.run(87, idle2);
  h.check(`${tag}: renderer on the device asked for`, cart.v('gpu.core') === (featureLevel === 'core' ? 1 : 0), `gpu.core ${cart.v('gpu.core')}`);
  const title = await cart.grab();
  h.savePng(shots, 'title.png', title);
  h.check(`${tag}: boots to the title`, cart.v('mode') === MODE.title, `mode ${cart.v('mode')}`);
  const shades = h.colours(title);
  h.check(`${tag}: title is a lit scene (many shades)`, shades > 150, `${shades} colours`);
  h.control(`${tag}: black frame as a lit scene`, h.colours(h.black(title)) > 150);
  // Grass green dominates the meadow, sky above it.
  const green = h.countWhere(title, (r, g, b) => g > r + 15 && g > b + 15);
  h.check(`${tag}: the meadow is green`, green > title.w * title.h * 0.25, `${green} green px`);
  h.control(`${tag}: black frame as a meadow`, h.countWhere(h.black(title), (r, g, b) => g > r + 15 && g > b + 15) > title.w * title.h * 0.25);
  await cart.run(600, idle2);
  h.check(`${tag}: ten idle seconds do not start a game`, cart.v('mode') === MODE.title, `mode ${cart.v('mode')}`);
  const title2 = await cart.grab();
  const titleMotion = h.changed(title, title2);
  h.check(`${tag}: the title camera moves`, titleMotion > 20000, `${titleMotion} px changed`);
  h.control(`${tag}: a frame compared with itself as motion`, h.changed(title2, title2) > 20000);

  // A starts field 1 (the field card), A again goes.
  cart.audioRms();
  await cart.run(3, [A, h.idle()]);
  await cart.run(40, idle2);
  h.check(`${tag}: A on pad 1 opens field 1`, cart.v('mode') === MODE.intro && cart.v('field') === 0, `mode ${cart.v('mode')}`);
  await cart.run(3, [A, h.idle()]);
  await cart.run(3, idle2);
  h.check(`${tag}: A again starts herding`, cart.v('mode') === MODE.play, `mode ${cart.v('mode')}`);

  // The dog runs with the stick and the d-pad, from either pad.
  const x0 = cart.v('dog_x');
  await cart.run(40, [h.pad(0, { lx: 1 }), h.idle()]);
  const x1 = cart.v('dog_x');
  h.check(`${tag}: left stick runs right`, x1 - x0 > 4, `x ${x0.toFixed(2)} -> ${x1.toFixed(2)}`);
  await cart.run(40, [h.pad(h.BTN.LEFT), h.idle()]);
  const x2 = cart.v('dog_x');
  h.check(`${tag}: d-pad runs left`, x1 - x2 > 4, `x ${x1.toFixed(2)} -> ${x2.toFixed(2)}`);
  const z0 = cart.v('dog_z');
  await cart.run(30, [h.idle(), h.pad(h.BTN.UP)]);
  const z1 = cart.v('dog_z');
  h.check(`${tag}: a player on the second pad runs too (up = away)`, z0 - z1 > 3, `z ${z0.toFixed(2)} -> ${z1.toFixed(2)}`);
  await cart.run(30, idle2);
  const x3 = cart.v('dog_x');
  await cart.run(30, idle2);
  h.control(`${tag}: no input as running`, Math.abs(cart.v('dog_x') - x3) > 1, `x ${x3.toFixed(2)} -> ${cart.v('dog_x').toFixed(2)}`);
  // Holding B sneaks: much slower.
  const s0 = cart.v('dog_x');
  await cart.run(40, [h.pad(h.BTN.B | h.BTN.RIGHT), h.idle()]);
  const sneakDx = cart.v('dog_x') - s0;
  const sneaking = cart.v('sneak');
  await cart.run(10, idle2);
  const r0 = cart.v('dog_x');
  await cart.run(40, [h.pad(h.BTN.LEFT), h.idle()]);
  const runDx = r0 - cart.v('dog_x');
  h.check(`${tag}: hold B sneaks at under half speed`, sneaking === 1 && sneakDx > 0.5 && sneakDx < runDx * 0.5, `sneak ${sneakDx.toFixed(2)} vs run ${runDx.toFixed(2)}`);
  h.control(`${tag}: a run as a sneak`, runDx < sneakDx * 1.2, `run ${runDx.toFixed(2)}`);

  // A barks: counted, and it sounds.
  cart.audioRms();
  await cart.run(20, idle2);
  const quiet = cart.audioRms();
  const b0 = cart.v('barks');
  await cart.run(2, [A, h.idle()]);
  await cart.run(18, idle2);
  const bark = cart.audioRms();
  h.check(`${tag}: A barks`, cart.v('barks') === b0 + 1, `barks ${b0} -> ${cart.v('barks')}`);
  h.check(`${tag}: the bark is louder than the music alone`, bark.rms > quiet.rms * 2.5 && bark.rms > 0.12, `rms ${bark.rms.toFixed(3)} vs ${quiet.rms.toFixed(3)}`);
  await cart.run(40, idle2);
  cart.audioRms();
  await cart.run(20, idle2);
  const after = cart.audioRms();
  h.control(`${tag}: twenty quiet frames as a bark`, after.rms > quiet.rms * 2.5 && after.rms > 0.12, `rms ${after.rms.toFixed(3)}`);

  // Start pauses (the sun stops), Start resumes.
  await cart.run(2, [START, h.idle()]);
  await cart.run(2, idle2);
  const tl = cart.v('time_left');
  await cart.run(60, idle2);
  h.check(`${tag}: Start pauses`, cart.v('mode') === MODE.paused && cart.v('time_left') === tl, `mode ${cart.v('mode')}`);
  await cart.run(2, [START, h.idle()]);
  await cart.run(30, idle2);
  h.check(`${tag}: Start resumes`, cart.v('mode') === MODE.play && cart.v('time_left') < tl);

  // Play motion: the dog, sheep and camera move between frames.
  const f1 = await cart.grab();
  await cart.run(20, [h.pad(0, { lx: 1, ly: 0.4 }), h.idle()]);
  const f2 = await cart.grab();
  const moved = h.changed(f1, f2);
  h.check(`${tag}: play moves between frames`, moved > 20000, `${moved} px changed`);
  h.savePng(shots, 'play-1.png', f2);

  // The autopilot herds field 1 with the idle second pad connected.
  cart.host.writeDebugValue('autopilot', 1);
  cart.audioRms();
  const frames = await runUntil(cart, 10800, idle2, () => cart.v('mode') !== MODE.play);
  h.check(`${tag}: the autopilot pens field 1 before sundown`, cart.v('mode') === MODE.cleared && cart.v('penned') >= cart.v('need'),
    `mode ${cart.v('mode')} penned ${cart.v('penned')}/${cart.v('need')} after ${frames} frames, sun ${cart.v('time_left').toFixed(0)} s`);
  const audio = cart.audioRms();
  const audible = (a) => a.rms > 0.02 && a.samples > 1000;
  h.check(`${tag}: music and sounds play`, audible(audio) && audio.samples > 48000, `rms ${audio.rms.toFixed(3)} over ${audio.samples} samples`);
  h.control(`${tag}: the silent first frames as audio`, audible(early), `rms ${early.rms.toFixed(4)} over ${early.samples} samples`);
  h.savePng(shots, 'cleared.png', await cart.grab());
  // On to field 2.
  await runUntil(cart, 600, idle2, () => cart.v('field') === 1 && cart.v('mode') === MODE.play);
  await cart.run(600, idle2);
  h.check(`${tag}: field 2 follows`, cart.v('field') === 1 && cart.v('mode') === MODE.play, `field ${cart.v('field')} mode ${cart.v('mode')}`);
  h.savePng(shots, 'play-2.png', await cart.grab());
  cart.destroy();

  // Control: a dog that never moves pens nothing; night falls, A retries.
  const c2 = await h.boot(ctx.file, { featureLevel });
  await c2.run(30, idle2);
  await c2.run(3, [A, h.idle()]);
  await c2.run(60, idle2);
  await runUntil(c2, 11400, idle2, () => c2.v('mode') !== MODE.play && c2.v('mode') !== MODE.intro);
  h.control(`${tag}: a hands-off dog as clearing field 1`, c2.v('mode') === MODE.cleared, `mode ${c2.v('mode')} penned ${c2.v('penned')}`);
  h.check(`${tag}: sundown with too few sheep ends the field`, c2.v('mode') === MODE.over, `mode ${c2.v('mode')}`);
  const over = await c2.grab();
  h.savePng(shots, 'over.png', over);
  await c2.run(3, [A, h.idle()]);
  await c2.run(3, idle2);
  h.check(`${tag}: game over waits before accepting A`, c2.v('mode') === MODE.over);
  await c2.run(100, idle2);
  await c2.run(3, [A, h.idle()]);
  await c2.run(3, idle2);
  h.check(`${tag}: A tries the field again`, c2.v('mode') === MODE.intro && c2.v('field') === 0 && c2.v('penned') === 0 && c2.v('time_left') > 170,
    `mode ${c2.v('mode')} field ${c2.v('field')}`);
  const log = c2.log().filter((l) => /error|panic/i.test(l));
  h.check(`${tag}: no GPU errors or panics logged`, log.length === 0, log.slice(0, 2).join(' | '));
  c2.destroy();
}

export default async function check(ctx) {
  await onDevice(ctx, 'core');
  await onDevice(ctx, 'compatibility');
}
