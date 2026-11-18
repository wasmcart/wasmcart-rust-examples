// A simple autopilot for Hexbloom, used by the checks: it reads the cart's
// debug values and returns one pad. It plays the way the title screen tells
// a newcomer to: left stick to move, right stick to aim and fire, B to bomb.
import { BTN } from '../../scripts/lib/harness.mjs';

export const FIELDS = ['mode', 'wave', 'score', 'lives', 'bombs', 'enemies', 'px', 'py', 'near_dx', 'near_dy', 'threat_dx', 'threat_dy', 'threat_d', 'first_bloom'];

/** @param {Record<string, number>} f debug values  @param {number} frame */
export function decide(f, frame) {
  if (f.mode !== 1) return { buttons: frame % 30 < 3 ? BTN.A : 0 };
  const cx = 480, cy = 290;
  let mx = 0, my = 0;
  const td = f.threat_d;
  if (td < 150) {
    // Step away from the closest threat, sideways so it does not pin us to a wall.
    mx = -f.threat_dx / td + 0.6 * (-f.threat_dy / td);
    my = -f.threat_dy / td + 0.6 * (f.threat_dx / td);
  } else {
    // Drift back toward the middle so there is always room.
    mx = (cx - f.px) / 300;
    my = (cy - f.py) / 200;
  }
  const l = Math.hypot(mx, my);
  if (l > 1) { mx /= l; my /= l; }
  const nd = Math.hypot(f.near_dx, f.near_dy);
  const rx = nd > 0 ? f.near_dx / nd : 0, ry = nd > 0 ? f.near_dy / nd : 0;
  let buttons = 0;
  if (td < 40 && f.bombs > 0 && frame % 20 === 0) buttons |= BTN.B;
  return { buttons, lx: mx, ly: my, rx, ry };
}
