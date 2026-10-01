// SPDX-License-Identifier: Apache-2.0
/**
 * Registry archive reader: ustar `prefix`, PAX `path=`, GNU `L` long names,
 * root-level selection, and every hard cap. Archives are built in memory —
 * no network, no fixtures on disk.
 */

import { describe, it, expect } from "vitest";
import { gzipSync } from "node:zlib";
import {
  ArchiveError,
  MAX_FILE_BYTES,
  fetchArchiveRootFiles,
  iterateTar,
  parsePaxHeaders,
  readRootTextFiles,
  type FetchFn,
} from "../live/package-archive.js";

interface TarSpec {
  name: string;
  body: string | Buffer;
  type?: string;
  prefix?: string;
}

function header(name: string, size: number, type: string, prefix = ""): Buffer {
  const h = Buffer.alloc(512);
  h.write(name.slice(0, 100), 0, "utf8");
  h.write("0000644\0", 100, "ascii");
  h.write("0000000\0", 108, "ascii");
  h.write("0000000\0", 116, "ascii");
  h.write(size.toString(8).padStart(11, "0") + "\0", 124, "ascii");
  h.write("00000000000\0", 136, "ascii");
  h.write(type, 156, "ascii");
  h.write("ustar\0", 257, "ascii");
  h.write("00", 263, "ascii");
  if (prefix) h.write(prefix, 345, "utf8");
  h.fill(0x20, 148, 156);
  let sum = 0;
  for (const b of h) sum += b;
  h.write(sum.toString(8).padStart(6, "0") + "\0 ", 148, "ascii");
  return h;
}

function pad(data: Buffer): Buffer {
  const rem = data.length % 512;
  return rem === 0 ? data : Buffer.concat([data, Buffer.alloc(512 - rem)]);
}

function paxRecord(key: string, value: string): string {
  const body = ` ${key}=${value}\n`;
  let len = Buffer.byteLength(body) + 1;
  while (String(len).length + Buffer.byteLength(body) !== len) len = String(len).length + Buffer.byteLength(body);
  return `${len}${body}`;
}

/** Build an uncompressed tar from specs (PAX/GNU entries are written as given). */
function buildTar(specs: TarSpec[]): Buffer {
  const parts: Buffer[] = [];
  for (const spec of specs) {
    const body = typeof spec.body === "string" ? Buffer.from(spec.body, "utf8") : spec.body;
    parts.push(header(spec.name, body.length, spec.type ?? "0", spec.prefix), pad(body));
  }
  parts.push(Buffer.alloc(1024));
  return Buffer.concat(parts);
}

const accept = (base: string) => /^(changelog|readme)/i.test(base);

describe("parsePaxHeaders", () => {
  it("reads byte-length-prefixed records including multi-byte values", () => {
    const data = Buffer.from(paxRecord("path", "package/ÇHANGELOG.md") + paxRecord("mtime", "1"));
    expect(parsePaxHeaders(data)).toEqual({ path: "package/ÇHANGELOG.md", mtime: "1" });
  });
});

describe("iterateTar", () => {
  it("joins the ustar prefix field with the name", () => {
    const tar = buildTar([{ name: "CHANGELOG.md", prefix: "package", body: "x" }]);
    expect([...iterateTar(tar)].map((e) => e.path)).toEqual(["package/CHANGELOG.md"]);
  });

  it("applies a PAX path= header to the next entry only", () => {
    const long = `package/${"a".repeat(120)}.md`;
    const tar = buildTar([
      { name: "PaxHeader", type: "x", body: paxRecord("path", long) },
      { name: "truncated-name", body: "one" },
      { name: "package/second.md", body: "two" },
    ]);
    expect([...iterateTar(tar)].map((e) => e.path)).toEqual([long, "package/second.md"]);
  });

  it("applies a GNU L long name to the next entry", () => {
    const long = `crate-1.0.0/${"b".repeat(150)}.txt`;
    const tar = buildTar([
      { name: "././@LongLink", type: "L", body: `${long}\0` },
      { name: "short", body: "data" },
    ]);
    const entries = [...iterateTar(tar)];
    expect(entries[0].path).toBe(long);
    expect(entries[0].data.toString()).toBe("data");
  });

  it("rejects a header whose checksum does not match", () => {
    const tar = buildTar([{ name: "package/a", body: "x" }]);
    tar[0] = "z".charCodeAt(0);
    expect(() => [...iterateTar(tar)]).toThrow(ArchiveError);
  });

  it("rejects an entry whose data runs past the archive", () => {
    const tar = buildTar([{ name: "package/a", body: "x".repeat(600) }]).subarray(0, 700);
    expect(() => [...iterateTar(tar)]).toThrow(/truncated/);
  });
});

describe("readRootTextFiles", () => {
  it("returns accepted root-level files and skips nested, binary and unaccepted ones", () => {
    const gz = gzipSync(
      buildTar([
        { name: "package/CHANGELOG.md", body: "## 1.0.0\n- a" },
        { name: "package/README.md", body: "hi" },
        { name: "package/docs/CHANGELOG.md", body: "nested" },
        { name: "package/changelog.bin", body: Buffer.from([0x43, 0, 0x44]) },
        { name: "package/index.js", body: "code" },
        { name: "package/dir", type: "5", body: "" },
      ]),
    );
    const files = readRootTextFiles(gz, accept);
    expect([...files.keys()].sort()).toEqual(["package/CHANGELOG.md", "package/README.md"]);
    expect(files.get("package/CHANGELOG.md")).toContain("## 1.0.0");
  });

  it("finds a root file whose name only fits in a PAX header", () => {
    const gz = gzipSync(
      buildTar([
        { name: "PaxHeader", type: "x", body: paxRecord("path", "fastembed-7.1.0/CHANGELOG.md") },
        { name: "x", body: "## 7.1.0" },
      ]),
    );
    expect(readRootTextFiles(gz, accept).get("fastembed-7.1.0/CHANGELOG.md")).toBe("## 7.1.0");
  });

  it("skips a single file over the per-file cap", () => {
    const gz = gzipSync(buildTar([{ name: "package/CHANGELOG.md", body: Buffer.alloc(MAX_FILE_BYTES + 1, 0x61) }]));
    expect(readRootTextFiles(gz, accept).size).toBe(0);
  });

  it("stops inflating a gzip bomb at the uncompressed cap", () => {
    // 81 MB of zeros compresses to ~80 KB: well under the compressed cap, over the uncompressed one.
    const bomb = gzipSync(Buffer.alloc(81 * 1024 * 1024));
    expect(() => readRootTextFiles(bomb, accept)).toThrow(/uncompressed/);
  });

  it("rejects input that is not gzip", () => {
    expect(() => readRootTextFiles(Buffer.from("not gzip"), accept)).toThrow(ArchiveError);
  });
});

describe("fetchArchiveRootFiles", () => {
  const archive = gzipSync(buildTar([{ name: "package/CHANGELOG.md", body: "## 2.0.0" }]));

  it("reads the archive through the injected fetch", async () => {
    const fetchFn: FetchFn = async () => new Response(archive);
    const files = await fetchArchiveRootFiles("https://registry.npmjs.org/x/-/x-2.0.0.tgz", accept, fetchFn);
    expect(files.get("package/CHANGELOG.md")).toBe("## 2.0.0");
  });

  it("refuses a declared Content-Length over the compressed cap without reading the body", async () => {
    const fetchFn: FetchFn = async () =>
      new Response(archive, { headers: { "content-length": String(21 * 1024 * 1024) } });
    await expect(fetchArchiveRootFiles("https://registry.npmjs.org/x.tgz", accept, fetchFn)).rejects.toThrow(/compressed/);
  });

  it("refuses a body that grows past the cap when no length is declared", async () => {
    const chunk = new Uint8Array(1024 * 1024);
    let sent = 0;
    const stream = new ReadableStream<Uint8Array>({
      pull(controller) {
        if (sent++ > 25) controller.close();
        else controller.enqueue(chunk);
      },
    });
    const fetchFn: FetchFn = async () => new Response(stream);
    await expect(fetchArchiveRootFiles("https://registry.npmjs.org/x.tgz", accept, fetchFn)).rejects.toThrow(/exceeds/);
  });

  it("reports an HTTP failure", async () => {
    const fetchFn: FetchFn = async () => new Response("nope", { status: 404 });
    await expect(fetchArchiveRootFiles("https://registry.npmjs.org/x.tgz", accept, fetchFn)).rejects.toThrow(/HTTP 404/);
  });
});
