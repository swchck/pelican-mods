/**
 * Compares dotted versions numerically ("4.10.0" > "4.9.1"); a prerelease suffix
 * ("4.5.0-beta.2") sorts before the release it precedes.
 */
export function compareVersions(a: string, b: string): number {
  const [coreA, preA] = split(a);
  const [coreB, preB] = split(b);
  for (let i = 0; i < Math.max(coreA.length, coreB.length); i++) {
    const d = (coreA[i] ?? 0) - (coreB[i] ?? 0);
    if (d !== 0) return Math.sign(d);
  }
  if (preA === preB) return 0;
  if (!preA) return 1;
  if (!preB) return -1;
  return preA < preB ? -1 : 1;
}

function split(v: string): [number[], string] {
  const clean = v.trim().replace(/^v/i, "").split("+")[0];
  const dash = clean.indexOf("-");
  const core = dash === -1 ? clean : clean.slice(0, dash);
  const pre = dash === -1 ? "" : clean.slice(dash + 1);
  return [core.split(".").map((n) => Number.parseInt(n, 10) || 0), pre];
}
