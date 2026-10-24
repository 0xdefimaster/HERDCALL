// Builds the Anchor IDL of the HerdCall program without the Anchor CLI: runs the program's IDL print tests
// (`idl-build` feature) and merges their output the way `anchor idl build` does. Run: npm run idl
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const programs = join(here, '..');
const out = execFileSync(
  'cargo',
  ['test', '-p', 'herdcall', '--features', 'idl-build', '__anchor_private_print_idl', '--', '--show-output', '--quiet'],
  { cwd: programs, env: { ...process.env, ANCHOR_IDL_BUILD_PROGRAM_PATH: join(programs, 'herdcall') }, encoding: 'utf8', maxBuffer: 64 << 20 },
);

const sections = [...out.matchAll(/--- IDL begin (\w+) ---\n([\s\S]*?)\n--- IDL end \1 ---/g)].map(([, kind, body]) => [kind, body]);
const one = (kind) => sections.find(([k]) => k === kind)?.[1];
const idl = JSON.parse(one('program'));
// The address section is the address string, quoted (sometimes twice).
let address = JSON.parse(one('address'));
if (address.startsWith('"')) address = JSON.parse(address);
idl.address = address;
const types = new Map(idl.types.map((t) => [t.name, t]));
const events = [];
for (const [kind, body] of sections) {
  if (kind !== 'event') continue;
  const { event, types: eventTypes } = JSON.parse(body);
  events.push(event);
  for (const t of eventTypes) types.set(t.name, t);
}
idl.events = events.sort((a, b) => a.name.localeCompare(b.name));
idl.errors = JSON.parse(one('errors'));
idl.types = [...types.values()].sort((a, b) => a.name.localeCompare(b.name));

// Module paths in type names (`herdcall::state::Config`) shortened to the last segment when that is unique, as the
// Anchor CLI does.
let json = JSON.stringify(idl, null, 2);
const full = [...new Set([...json.matchAll(/"([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)+)"/g)].map((m) => m[1]))];
const short = (n) => n.split('::').pop();
for (const n of full) {
  if (full.filter((m) => short(m) === short(n)).length === 1) json = json.replaceAll(`"${n}"`, `"${short(n)}"`);
}
writeFileSync(join(here, 'herdcall.json'), `${json}\n`);
console.log(`herdcall.json: ${idl.instructions.length} instructions, ${idl.accounts.length} accounts, ${idl.events.length} events, ${idl.errors.length} errors`);
