#!/usr/bin/env node
//
// Parse + VALIDATE tools/gate-jobs.json for .githooks/pre-push.
//
// WHY THIS FILE EXISTS
// --------------------
// The pre-push gate used to run each declared job with `eval "$cmd"`, where
// `$cmd` came verbatim out of tools/gate-jobs.json — a TRACKED file. Any merged
// pull request that edited one `cmd` string would then execute, as the
// developer, on every developer's machine, the next time they pushed. No
// allowlist, no argv form, no review step between "the string landed on main"
// and "the string ran on 12 laptops". `git pull` is not supposed to be a code
// execution primitive.
//
// The fix is structural, not a filter:
//   1. Commands are split into an ARGV VECTOR and executed with no shell, so
//      shell syntax in the JSON is not syntax — it is just a wrong argument.
//   2. Because splitting a string into argv means guessing at quoting, anything
//      that looks like it WANTS a shell is REFUSED LOUDLY rather than guessed
//      at. Today every declared job is a plain `<exe> <args...>` invocation, so
//      nothing legitimate is lost; if a job ever genuinely needs a pipe, it
//      belongs in a script file that the gate then invokes by name.
//   3. argv[0] must be on a short allowlist of build tooling. A job that wants
//      `curl`, `bash`, `powershell` or anything else is refused.
//
// EXIT CODES (the hook depends on these)
//   0  ok      — one line per job on stdout: `id \t arg0 \t arg1 ...`
//   2  REFUSED — the spec parsed but a job is unsafe or malformed. FAIL-CLOSED:
//                the hook must block the push. This is the attack case.
//   3  UNUSABLE — the spec is missing, unparseable or declares no jobs. The hook
//                keeps its historical FAIL-OPEN behaviour here (honoring
//                GATE_REQUIRE=1), because "the file is not there" is a broken
//                checkout, not an attempt to run something.

'use strict';

const fs = require('fs');

// Allowed command FORMS, not merely allowed executables.
//
// An argv[0] allowlist is not sufficient. `npx -y attacker-pkg` and `pnpm dlx
// attacker-pkg` contain no shell metacharacter, pass an argv[0] check
// unchanged, and fetch and execute arbitrary code from the registry — so a
// merged edit to the tracked tools/gate-jobs.json would still be a code
// execution primitive on every developer's machine at push time, which is the
// exact property this module exists to remove.
//
// Every job the gate actually declares is one of two shapes, so the validator
// pins those shapes instead:
//
//     pnpm run <script>            -> a script defined in this repo's package.json
//     node scripts/<name>.cjs [--flag ...]
//
// `npx`, `npm` and `cargo` are deliberately absent: no declared job uses them,
// and `npx`/`npm exec` are the registry-fetch forms above. Widening this is a
// reviewable change to THIS file, not a silent edit to a data file.
const ALLOWED_FORMS = [
  {
    label: 'pnpm run <script>',
    test: (argv) =>
      argv.length === 3 && argv[0] === 'pnpm' && argv[1] === 'run' && /^[a-z0-9][a-z0-9:_-]*$/.test(argv[2]),
  },
  {
    label: 'node scripts/<name>.cjs [--flag ...]',
    test: (argv) =>
      argv.length >= 2 &&
      argv[0] === 'node' &&
      /^scripts\/[a-z0-9][a-z0-9._-]*\.cjs$/.test(argv[1]) &&
      argv.slice(2).every((a) => /^--[a-z0-9][a-z0-9-]*$/.test(a)),
  },
];

// Anything that only means something to a shell. Present => refuse, do not try
// to interpret. Covers command substitution, chaining, redirection, globbing,
// quoting, variable expansion, background/subshell and comments.
const SHELL_METACHARACTERS = /[`$&;|<>(){}[\]*?!#~'"\\\n\r\t]/;

const REFUSE = 2;
const UNUSABLE = 3;

class Refusal extends Error {}

/**
 * Turn one declared `cmd` string into a validated argv vector.
 * @param {string} id  job id, for diagnostics
 * @param {unknown} cmd
 * @returns {string[]} argv
 */
function toArgv(id, cmd) {
  if (typeof cmd !== 'string' || cmd.trim() === '') {
    throw new Refusal(`job "${id}": cmd must be a non-empty string`);
  }
  if (SHELL_METACHARACTERS.test(cmd)) {
    const offending = cmd.match(SHELL_METACHARACTERS)[0];
    throw new Refusal(
      `job "${id}": cmd contains the shell metacharacter ${JSON.stringify(offending)} — ` +
        `refused.\n` +
        `    cmd: ${JSON.stringify(cmd)}\n` +
        `    The gate runs jobs as an argv vector with NO shell. If this job genuinely ` +
        `needs shell\n    features, put them in a script and declare the script here ` +
        `instead.`,
    );
  }
  const argv = cmd.trim().split(/ +/);
  if (!ALLOWED_FORMS.some((form) => form.test(argv))) {
    throw new Refusal(
      `job "${id}": cmd does not match an allowed command form — refused.\n` +
        `    cmd: ${JSON.stringify(cmd)}\n` +
        `    allowed: ${ALLOWED_FORMS.map((f) => f.label).join('  |  ')}\n` +
        `    tools/gate-jobs.json is a tracked file that runs on every developer's ` +
        `machine at push\n    time, so the gate pins whole command shapes rather than ` +
        `just the executable:\n    "npx -y attacker-pkg" has no shell metacharacter and ` +
        `would pass an executable-only\n    allowlist. Widening this is a reviewable ` +
        `change to scripts/gate-jobs.cjs.`,
    );
  }
  return argv;
}

/**
 * Validate a parsed spec object.
 * @param {unknown} spec
 * @returns {{id: string, argv: string[]}[]}
 */
function validateSpec(spec) {
  if (!spec || typeof spec !== 'object' || !Array.isArray(spec.jobs)) {
    throw new Refusal('spec has no `jobs` array');
  }
  return spec.jobs.map((job, i) => {
    if (!job || typeof job !== 'object') throw new Refusal(`jobs[${i}] is not an object`);
    const id = job.id;
    // The hook reads `id \t argv...`, so a tab or newline in the id would
    // silently shift every field after it.
    if (typeof id !== 'string' || id === '' || /[\t\n\r]/.test(id)) {
      throw new Refusal(`jobs[${i}] has a missing or unusable id: ${JSON.stringify(id)}`);
    }
    return { id, argv: toArgv(id, job.cmd) };
  });
}

/**
 * Read and validate a spec file.
 * @param {string} specPath
 * @returns {{id: string, argv: string[]}[]}
 */
function loadJobs(specPath) {
  let raw;
  try {
    raw = fs.readFileSync(specPath, 'utf8');
  } catch (err) {
    const e = new Error(`cannot read ${specPath}: ${err.message}`);
    e.unusable = true;
    throw e;
  }
  let spec;
  try {
    spec = JSON.parse(raw);
  } catch (err) {
    const e = new Error(`cannot parse ${specPath}: ${err.message}`);
    e.unusable = true;
    throw e;
  }
  const jobs = validateSpec(spec);
  if (jobs.length === 0) {
    const e = new Error(`${specPath} declares no jobs`);
    e.unusable = true;
    throw e;
  }
  return jobs;
}

function main(argv) {
  const specPath = argv[argv.indexOf('--print-argv') + 1];
  if (!argv.includes('--print-argv') || !specPath) {
    process.stderr.write('usage: node scripts/gate-jobs.cjs --print-argv <tools/gate-jobs.json>\n');
    return UNUSABLE;
  }
  let jobs;
  try {
    jobs = loadJobs(specPath);
  } catch (err) {
    process.stderr.write(`[gate-jobs] ${err.message}\n`);
    if (err instanceof Refusal) {
      process.stderr.write('[gate-jobs] REFUSED — the gate spec is not safe to execute.\n');
      return REFUSE;
    }
    return UNUSABLE;
  }
  process.stdout.write(jobs.map((j) => [j.id, ...j.argv].join('\t')).join('\n') + '\n');
  return 0;
}

if (require.main === module) {
  process.exitCode = main(process.argv.slice(2));
}

module.exports = {
  ALLOWED_FORMS,
  Refusal,
  loadJobs,
  main,
  toArgv,
  validateSpec,
  EXIT: { OK: 0, REFUSE, UNUSABLE },
};
