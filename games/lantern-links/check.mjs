// Lantern Links' cart checks: played with pad input on a core WebGPU device
// and in compatibility mode. Each check has a control that must fail.
const MODE = { title: 0, intro: 1, aim: 2, roll: 3, sunk: 4, paused: 5, card: 6 };

async function runUntil(cart, max, pads, pred, step = 10) {
  for (let i = 0; i < max; i += step) {
    await cart.run(step, pads);
    if (pred()) return i + step;
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

  await cart.run(3, idle2);
  const early = cart.audioRms();
  await cart.run(87, idle2);
  h.check(`${tag}: renderer on the device asked for`, cart.v('gpu.core') === (featureLevel === 'core' ? 1 : 0), `gpu.core ${cart.v('gpu.core')}`);
  const title = await cart.grab();
  h.savePng(shots, 'title.png', title);
  h.check(`${tag}: boots to the title`, cart.v('mode') === MODE.title, `mode ${cart.v('mode')}`);
  const isGreen = (r, g, b) => g > r + 25 && g > b + 15 && g > 50;
  const green = h.countWhere(title, isGreen);
  h.check(`${tag}: the course is drawn (green)`, green > 9000, `${green} green px`);
  h.control(`${tag}: black frame as a course`, h.countWhere(h.black(title), isGreen) > 9000);
  const shades = h.colours(title);
  h.check(`${tag}: lantern light shades the course`, shades > 120, `${shades} colours`);
  h.control(`${tag}: black frame as shaded`, h.colours(h.black(title)) > 120);
  await cart.run(600, idle2);
  h.check(`${tag}: ten idle seconds do not start a round`, cart.v('mode') === MODE.title);
  const title2 = await cart.grab();
  const drift = h.changed(title, title2);
  h.check(`${tag}: fireflies drift on the title`, drift > 300, `${drift} px changed`);
  h.control(`${tag}: a frame compared with itself as motion`, h.changed(title2, title2) > 300);

  await cart.run(3, [A, h.idle()]);
  await cart.run(20, idle2);
  h.check(`${tag}: A on pad 1 opens hole 1`, cart.v('mode') === MODE.intro && cart.v('hole') === 0, `mode ${cart.v('mode')}`);
  await cart.run(10, idle2);
  await cart.run(3, [A, h.idle()]);
  await cart.run(20, idle2);
  h.check(`${tag}: A again goes to the tee`, cart.v('mode') === MODE.aim, `mode ${cart.v('mode')}`);

  // Aiming: stick points it, d-pad turns it, the second pad works too.
  await cart.run(5, [h.pad(0, { lx: 0, ly: 1 }), h.idle()]);
  const down = cart.v('aim');
  h.check(`${tag}: stick down aims down`, Math.abs(down - Math.PI / 2) < 0.05, `aim ${down.toFixed(3)}`);
  await cart.run(30, [h.pad(h.BTN.RIGHT), h.idle()]);
  const turned = cart.v('aim');
  h.check(`${tag}: d-pad right turns the aim`, turned - down > 0.6, `aim ${down.toFixed(2)} -> ${turned.toFixed(2)}`);
  await cart.run(30, [h.idle(), h.pad(h.BTN.LEFT)]);
  const back = cart.v('aim');
  h.check(`${tag}: the second pad turns it back`, turned - back > 0.6, `aim ${turned.toFixed(2)} -> ${back.toFixed(2)}`);
  await cart.run(30, [h.pad(h.BTN.B | h.BTN.RIGHT), h.idle()]);
  const fine = cart.v('aim') - back;
  h.check(`${tag}: hold B aims finely`, fine > 0.05 && fine < 0.3, `turned ${fine.toFixed(3)} in half a second`);
  await cart.run(30, idle2);
  const still = cart.v('aim');
  await cart.run(30, idle2);
  h.control(`${tag}: no input as aiming`, Math.abs(cart.v('aim') - still) > 0.01);
  // Aim dots are drawn ahead of the ball (pale yellow).
  await cart.run(5, [h.pad(0, { lx: 1, ly: 0 }), h.idle()]);
  const aimed = await cart.grab();
  const bx = Math.round(cart.v('ball_x')), by = Math.round(cart.v('ball_y')) + 30;
  const isDot = (r, g, b) => r > 200 && g > 190 && b > 120 && b < 230;
  const dots = h.countWhere(aimed, isDot, { x: bx + 12, y: by - 8, w: 140, h: 16 });
  h.check(`${tag}: aim dots lead from the ball`, dots > 12, `${dots} dot px`);
  const dotsBehind = h.countWhere(aimed, isDot, { x: Math.max(0, bx - 152), y: by - 8, w: 140, h: 16 });
  h.control(`${tag}: the same count behind the ball as dots`, dotsBehind > 12, `${dotsBehind} px`);
  h.savePng(shots, 'play-1.png', aimed);

  // Power: hold A charges, B cancels; a release putts.
  await cart.run(33, [A, h.idle()]);
  const p = cart.v('power');
  h.check(`${tag}: holding A charges the putt`, cart.v('charging') === 1 && p > 0.4 && p < 0.6, `power ${p.toFixed(2)}`);
  await cart.run(2, [h.pad(h.BTN.A | h.BTN.B), h.idle()]);
  await cart.run(5, idle2);
  h.check(`${tag}: B cancels the charge`, cart.v('charging') === 0 && cart.v('strokes') === 0 && cart.v('mode') === MODE.aim);
  const x0 = cart.v('ball_x');
  await cart.run(26, [A, h.idle()]);
  cart.audioRms();
  await cart.run(4, [A, h.idle()]);
  const before = cart.audioRms();
  await cart.run(4, idle2);
  const putt = cart.audioRms();
  h.check(`${tag}: releasing A putts`, cart.v('strokes') === 1 && cart.v('ball_x') - x0 > 20, `strokes ${cart.v('strokes')} ball ${x0.toFixed(0)} -> ${cart.v('ball_x').toFixed(0)}`);
  h.check(`${tag}: the putt sounds over the music`, putt.rms > before.rms * 1.8 && putt.rms > 0.09, `rms ${putt.rms.toFixed(3)} vs ${before.rms.toFixed(3)}`);
  const r1 = await cart.grab();
  await cart.run(4, idle2);
  const r2 = await cart.grab();
  h.check(`${tag}: the rolling ball moves on screen`, h.changed(r1, r2) > 40, `${h.changed(r1, r2)} px`);
  await runUntil(cart, 600, idle2, () => cart.v('mode') !== MODE.roll);
  await cart.run(20, idle2);
  cart.audioRms();
  await cart.run(4, idle2);
  const later = cart.audioRms();
  h.control(`${tag}: four quiet frames at rest as a putt`, later.rms > before.rms * 1.8 && later.rms > 0.09, `rms ${later.rms.toFixed(3)}`);
  h.check(`${tag}: the ball comes to rest for the next putt`, cart.v('mode') === MODE.aim || cart.v('mode') === MODE.sunk, `mode ${cart.v('mode')}`);

  // Pause.
  if (cart.v('mode') === MODE.aim) {
    await cart.run(2, [START, h.idle()]);
    await cart.run(2, idle2);
    h.check(`${tag}: Start pauses`, cart.v('mode') === MODE.paused);
    await cart.run(2, [START, h.idle()]);
    await cart.run(30, idle2);
    h.check(`${tag}: Start resumes`, cart.v('mode') === MODE.aim);
  }

  // The autopilot plays the whole round with the idle second pad connected.
  cart.host.writeDebugValue('autopilot', 1);
  let shot2 = false;
  const frames = await runUntil(cart, 12000, idle2, () => {
    return cart.v('mode') === MODE.card || (!shot2 && cart.v('hole') === 6 && cart.v('mode') === MODE.roll);
  }, 10);
  if (cart.v('mode') !== MODE.card) {
    await cart.run(8, idle2);
    h.savePng(shots, 'play-2.png', await cart.grab());
    shot2 = true;
    await runUntil(cart, 12000, idle2, () => cart.v('mode') === MODE.card, 10);
  }
  cart.host.writeDebugValue('autopilot', 0);
  const done = cart.v('holes_done');
  h.check(`${tag}: the autopilot finishes all nine holes`, cart.v('mode') === MODE.card && done === 9,
    `holes ${done}, sunk ${cart.v('sunk')}, ${cart.v('total')} strokes (${cart.v('to_par')} to par) after ${frames} frames`);
  h.check(`${tag}: every hole is sinkable (none picked up)`, cart.v('sunk') >= 9, `sunk ${cart.v('sunk')}`);
  await cart.run(150, idle2);
  h.savePng(shots, 'card.png', await cart.grab());
  const audio = cart.audioRms();
  const audible = (a) => a.rms > 0.02 && a.samples > 1000;
  h.check(`${tag}: music and sounds play`, audible(audio), `rms ${audio.rms.toFixed(3)} over ${audio.samples} samples`);
  h.control(`${tag}: the silent first frames as audio`, audible(early), `rms ${early.rms.toFixed(4)}`);
  await cart.run(3, [A, h.idle()]);
  await cart.run(5, idle2);
  h.check(`${tag}: A on the scorecard goes back to the title`, cart.v('mode') === MODE.title, `mode ${cart.v('mode')}`);
  const log = cart.log().filter((l) => /error|panic/i.test(l));
  h.check(`${tag}: no GPU errors or panics logged`, log.length === 0, log.slice(0, 2).join(' | '));
  cart.destroy();

  // Control: nobody putts, nothing is sunk.
  const c2 = await h.boot(ctx.file, { featureLevel });
  await c2.run(30, idle2);
  await c2.run(3, [A, h.idle()]);
  await c2.run(900, idle2);
  h.control(`${tag}: a round nobody plays as sinking holes`, c2.v('sunk') > 0 || c2.v('strokes') > 0, `sunk ${c2.v('sunk')} strokes ${c2.v('strokes')}`);
  c2.destroy();
}

export default async function check(ctx) {
  await onDevice(ctx, 'core');
  await onDevice(ctx, 'compatibility');
}
