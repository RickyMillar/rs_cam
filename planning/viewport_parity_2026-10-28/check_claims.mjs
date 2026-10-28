#!/usr/bin/env node
// Checks every claim that viewport_parity.html makes (C1-C8). It extracts
// the page's own camera-model script and runs it, so the checked code is
// the drawn code. Run from anywhere: node check_claims.mjs
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..");
const html = readFileSync(join(here, "viewport_parity.html"), "utf8");

function block(id) {
  const m = html.match(new RegExp(`<script id="${id}"[^>]*>([\\s\\S]*?)</script>`));
  if (!m) throw new Error(`no <script id="${id}"> block`);
  return m[1];
}
const sandbox = { module: { exports: {} } };
vm.runInNewContext(block("camera-model"), sandbox);
const M = sandbox.module.exports;

let failures = 0;
function check(name, ok, detail = "") {
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? "  " + detail : ""}`);
  if (!ok) failures += 1;
}
const W = 520, H = 300;
// A fixed pseudo-random sequence, so the run is repeatable.
let seed = 12345;
const rnd = () => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed / 0x7fffffff; };
const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1]);

// C1: zoom to cursor keeps the point under the pointer fixed; the centre
// zoom moves a point at the target depth by r * |1/k - 1| (r = its
// distance from the screen centre).
{
  let maxFixed = 0, maxErr = 0, minCentre = Infinity;
  for (let i = 0; i < 40; i++) {
    const cam = M.make({ yaw: rnd() * 6.28, pitch: 0.2 + rnd() * 1.2, distance: 100 + rnd() * 400 });
    const sx = 40 + rnd() * (W - 80), sy = 40 + rnd() * (H - 80);
    const k = 0.5 + rnd();
    const P = M.pointAtTargetDepth(cam, sx, sy, W, H);
    const before = M.project(cam, P, W, H);
    const afterCursor = M.project(M.zoomToCursor(cam, k, sx, sy, W, H), P, W, H);
    const afterCentre = M.project(M.zoomCentre(cam, k), P, W, H);
    maxFixed = Math.max(maxFixed, dist(before, afterCursor));
    const r = dist(before, [W / 2, H / 2]);
    const expected = r * Math.abs(1 / k - 1);
    maxErr = Math.max(maxErr, Math.abs(dist(before, afterCentre) - expected));
    if (r > 40 && Math.abs(k - 1) > 0.1) minCentre = Math.min(minCentre, dist(before, afterCentre));
  }
  check("C1a zoom-to-cursor keeps the pointer point fixed", maxFixed < 1e-6, `max drift ${maxFixed.toExponential(2)} px`);
  check("C1b centre zoom moves a point by r * |1/k - 1|", maxErr < 1e-6 && minCentre > 1, `max error ${maxErr.toExponential(2)} px; min move at r > 40 px, |k-1| > 0.1: ${minCentre.toFixed(1)} px`);
}

// C2: orbit about the pivot keeps the pivot fixed; the fixed-target orbit moves it.
// C3: the pivot alone changes no pose.
{
  let maxFixed = 0, minMoved = Infinity, maxPose = 0;
  for (let i = 0; i < 40; i++) {
    const cam = M.make({ yaw: rnd() * 6.28, pitch: 0.2 + rnd() * 1.0, distance: 150 + rnd() * 300 });
    const sx = 40 + rnd() * (W - 80), sy = 40 + rnd() * (H - 80);
    const Q = M.hitGround(cam, sx, sy, W, H) || M.pointAtTargetDepth(cam, sx, sy, W, H);
    const before = M.project(cam, Q, W, H);
    let a = cam, b = cam;
    for (let s = 0; s < 10; s++) {   // a 10-step drag of random deltas
      const dx = (rnd() - 0.5) * 40, dy = (rnd() - 0.5) * 20;
      a = M.orbitAboutPivot(a, Q, dx, dy);
      b = M.orbitFixed(b, dx, dy);
    }
    maxFixed = Math.max(maxFixed, dist(before, M.project(a, Q, W, H)));
    if (dist(Q, cam.target) > 5) minMoved = Math.min(minMoved, dist(before, M.project(b, Q, W, H)));
    // C3: a zero step about the pivot leaves eye and forward unchanged.
    const z = M.orbitAboutPivot(cam, Q, 0, 0);
    const e0 = M.eye(cam), e1 = M.eye(z), f0 = M.basis(cam).f, f1 = M.basis(z).f;
    maxPose = Math.max(maxPose, Math.hypot(...e0.map((v, j) => v - e1[j])), Math.hypot(...f0.map((v, j) => v - f1[j])));
  }
  check("C2a orbit about the pivot keeps it fixed through a drag", maxFixed < 1e-6, `max drift ${maxFixed.toExponential(2)} px`);
  check("C2b fixed-target orbit moves the pivot point", minMoved > 1, `min move ${minMoved.toFixed(1)} px`);
  check("C3 the pivot alone changes no pose", maxPose < 1e-9, `max change ${maxPose.toExponential(2)}`);
}

// C4: the tween starts at a, ends at b, moves monotonically.
{
  const a = M.make(), b = M.preset(M.make({ target: [10, 20, 5], distance: 300 }), "top");
  const t0 = M.tween(a, b, 0), t1 = M.tween(a, b, 1);
  const same = (x, y) => Math.abs(x.distance - y.distance) < 1e-9 && Math.abs(x.yaw - y.yaw) < 1e-9 && Math.abs(x.pitch - y.pitch) < 1e-9 && dist(x.target, y.target) < 1e-9;
  let mono = true, prev = -1;
  for (let i = 0; i <= 100; i++) { const e = M.easeInOut(i / 100); if (e < prev) mono = false; prev = e; }
  check("C4 tween: start = old pose, end = new pose, monotone", same(t0, a) && same(t1, b) && mono);
}

// C5: hover rebuild bytes = 108 per triangle; the terrain figure.
{
  const b = M.hoverRebuildBytes(131962);
  check("C5 hover rebuild bytes", b === 131962 * 120 && b === 15835440, `${b} B`);
}

// C6: the probe numbers in the page equal probe/run.log.
{
  const log = readFileSync(join(here, "probe", "run.log"), "utf8");
  const rows = [...html.matchAll(/<tr><td>([^<]+)<\/td><td class="num">(\d+)<\/td><td class="num">([\d.]+)<\/td><td class="num">([\d.]+)<\/td><td class="num">([\d.]+)<\/td><\/tr>/g)];
  check("C6a the page holds five probe rows", rows.length === 5, `${rows.length} rows`);
  const blocks = log.split(/\n(?=\S)/).filter(b => /triangles,/.test(b)).map(b => b.split('\n').filter(l => !/DISAGREE/.test(l)).join('\n'));
  let ok = rows.length === blocks.length;
  rows.forEach((r, i) => {
    const b = blocks[i] || "";
    const tris = b.match(/(\d+) triangles/)?.[1];
    const bf = b.match(/brute force ray_pick_triangle: mean ([\d.]+) ms/)?.[1];
    const cast = b.match(/cast: mean ([\d.]+) ms/)?.[1];
    const build = b.match(/BVH build: ([\d.]+) ms/)?.[1];
    const rowOk = tris === r[2] && bf === r[3] && cast === r[4] && build === r[5];
    if (!rowOk) ok = false;
    check(`C6b row ${i + 1} (${r[1]}) matches run.log`, rowOk, rowOk ? "" : `log: ${tris} ${bf} ${cast} ${build}`);
  });
  check("C6 all probe numbers match", ok);
}

// C7: each cited file:line holds the named symbol.
{
  const claims = JSON.parse(block("code-claims"));
  for (const c of claims) {
    let line = "";
    try { line = readFileSync(join(repo, c.file), "utf8").split("\n")[c.line - 1] ?? ""; } catch { line = ""; }
    check(`C7 ${c.file}:${c.line} holds "${c.contains}"`, line.includes(c.contains), line.includes(c.contains) ? "" : `got: ${line.trim().slice(0, 80)}`);
  }
}

// C8: the two pan rules agree at one viewport height only (828 px at 45°).
{
  const cam = M.make();
  const hAgree = M.panAgreeHeight(cam);
  const at = (h) => { const a = M.panNow(cam, 10, 0), b = M.panDerived(cam, 10, 0, W, h); return dist(a.target, b.target); };
  check("C8a the pan rules agree at 828 px (45° fov)", Math.round(hAgree) === 828 && at(hAgree) < 1e-9, `agree height ${hAgree.toFixed(1)} px, difference there ${at(hAgree).toExponential(1)} mm`);
  check("C8b the pan rules differ at 300 px and at 1400 px", at(300) > 0.1 && at(1400) > 0.1, `${at(300).toFixed(2)} mm, ${at(1400).toFixed(2)} mm per 10 px`);
}

console.log(failures === 0 ? "\nALL CLAIMS HOLD" : `\n${failures} CLAIM(S) FAIL`);
process.exit(failures === 0 ? 0 : 1);
