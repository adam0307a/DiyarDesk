// Local preview server & Theme SDK validator:
//   node prototype/serve.mjs                   -> http://127.0.0.1:5178/settings.html
//   node prototype/serve.mjs --validate <path> -> validates a theme directory or .zip/.deskland package
import { createServer } from 'node:http';
import { readdir, readFile, stat } from 'node:fs/promises';
import { basename, extname, join, normalize, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('.', import.meta.url));
const rootPrefix = root.endsWith(sep) ? root : root + sep;
const types = {
  '.html': 'text/html; charset=utf-8',
  '.htm': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.mp3': 'audio/mpeg',
  '.ogg': 'audio/ogg',
  '.wav': 'audio/wav',
  '.mp4': 'video/mp4',
  '.webm': 'video/webm',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.wasm': 'application/wasm',
  '.glsl': 'text/plain; charset=utf-8',
};
const ALLOWED_EXTS = new Set([
  '.html', '.htm', '.css', '.js', '.mjs', '.json',
  '.glsl', '.vert', '.frag', '.wgsl', '.wasm',
  '.txt', '.md', '.xml', '.csv', '.map',
  '.png', '.jpg', '.jpeg', '.webp', '.gif', '.svg', '.avif', '.bmp', '.ico',
  '.mp4', '.webm', '.mkv', '.mov', '.ogv',
  '.mp3', '.wav', '.ogg', '.opus', '.m4a', '.aac', '.flac',
  '.woff', '.woff2', '.ttf', '.otf', '.eot',
  '.obj', '.mtl', '.gltf', '.glb', '.bin', '.hdr', '.exr', '.ktx', '.ktx2', '.dds', '.ply', '.stl', '.fbx',
]);
const ALLOWED_BASENAMES = new Set(['license', 'notice', 'readme', 'copying', 'authors', 'changelog']);
const MAX_ARCHIVE_BYTES = 150 * 1024 * 1024;
const MAX_ARCHIVE_FILES = 2000;
const EXT_URL_RE = /https?:\/\/[^\s"'<>)]+/gi;

const csp =
  "default-src 'self' data: blob:; " +
  "script-src 'self' 'unsafe-inline' 'unsafe-eval' data: blob:; " +
  "style-src 'self' 'unsafe-inline' data: blob:; " +
  "img-src 'self' data: blob:; " +
  "media-src 'self' data: blob:; " +
  "font-src 'self' data: blob:; " +
  "connect-src 'self' data: blob:; " +
  "frame-src 'self' data: blob:; " +
  "object-src 'none'; base-uri 'self'; form-action 'none'";

function isAllowedMember(fileName) {
  const ext = extname(fileName).toLowerCase();
  if (ext) return ALLOWED_EXTS.has(ext);
  return ALLOWED_BASENAMES.has(fileName.toLowerCase());
}

function parseZipCentralDirectory(buf) {
  // Locate End of Central Directory (0x06054b50)
  const minPos = Math.max(0, buf.length - 65557);
  let eocd = -1;
  for (let i = buf.length - 22; i >= minPos; i--) {
    if (buf.readUInt32LE(i) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) throw new Error('Not a valid ZIP archive (EOCD not found).');
  const cdSize = buf.readUInt32LE(eocd + 12);
  const cdOffset = buf.readUInt32LE(eocd + 16);
  const entries = [];
  let pos = cdOffset;
  const end = cdOffset + cdSize;
  while (pos + 46 <= end && pos + 46 <= buf.length) {
    if (buf.readUInt32LE(pos) !== 0x02014b50) break;
    const uncompSize = buf.readUInt32LE(pos + 24);
    const nameLen = buf.readUInt16LE(pos + 28);
    const extraLen = buf.readUInt16LE(pos + 30);
    const commentLen = buf.readUInt16LE(pos + 32);
    const extAttrs = buf.readUInt32LE(pos + 38);
    const name = buf.subarray(pos + 46, pos + 46 + nameLen).toString('utf8');
    const isSymlink = (((extAttrs >>> 16) & 0xf000) === 0xa000);
    entries.push({ name, size: uncompSize, isSymlink });
    pos += 46 + nameLen + extraLen + commentLen;
  }
  return entries;
}

async function walkDir(dir, baseDir, out = []) {
  const items = await readdir(dir, { withFileTypes: true });
  for (const it of items) {
    if (it.name === '.git' || it.name === '__MACOSX' || it.name === 'node_modules') continue;
    const full = join(dir, it.name);
    if (it.isSymbolicLink()) {
      out.push({ rel: full.slice(baseDir.length + 1).replace(/\\/g, '/'), full, size: 0, isSymlink: true });
    } else if (it.isDirectory()) {
      await walkDir(full, baseDir, out);
    } else if (it.isFile()) {
      const st = await stat(full);
      out.push({ rel: full.slice(baseDir.length + 1).replace(/\\/g, '/'), full, size: st.size, isSymlink: false });
    }
  }
  return out;
}

async function validateTarget(targetArg) {
  const target = resolve(targetArg);
  const st = await stat(target);
  const errors = [];
  const warnings = [];
  let fileCount = 0;
  let totalBytes = 0;
  let htmlFiles = [];

  if (st.isDirectory()) {
    const files = await walkDir(target, target);
    for (const f of files) {
      if (f.isSymlink) { errors.push(`Symbolic link not allowed: ${f.rel}`); continue; }
      fileCount++;
      totalBytes += f.size;
      const base = basename(f.rel);
      if (!isAllowedMember(base)) errors.push(`Unsupported file type: ${f.rel}`);
      const ext = extname(base).toLowerCase();
      if (ext === '.html' || ext === '.htm') htmlFiles.push(f.rel);
      if (['.json'].includes(ext) && ['manifest.json', 'theme.json', 'livelyinfo.json', 'livelyproperties.json'].includes(base.toLowerCase())) {
        try { JSON.parse(await readFile(f.full, 'utf8')); }
        catch (e) { errors.push(`Invalid JSON in ${f.rel}: ${e.message}`); }
      }
      if (['.html', '.htm', '.js', '.css'].includes(ext) && f.size <= 2 * 1024 * 1024) {
        const text = await readFile(f.full, 'utf8');
        const matches = (text.match(EXT_URL_RE) || []).filter(u =>
          !/^https?:\/\/(www\.w3\.org|app\.local|custom\.local|audio\.local|127\.0\.0\.1|localhost)/i.test(u)
        );
        if (matches.length > 0) {
          warnings.push(`${f.rel}: contains external URL (${matches[0].slice(0, 64)}...) - blocked by DiyarDesk offline CSP.`);
        }
      }
    }
  } else {
    const ext = extname(target).toLowerCase();
    if (ext !== '.zip' && ext !== '.deskland') {
      errors.push(`Expected a directory, .zip, or .deskland package (got ${ext || 'none'}).`);
    } else {
      const buf = await readFile(target);
      const entries = parseZipCentralDirectory(buf);
      for (const e of entries) {
        const norm = e.name.replace(/\\/g, '/');
        if (norm.endsWith('/')) continue;
        if (e.isSymlink) { errors.push(`Symbolic link in archive: ${e.name}`); continue; }
        if (norm.startsWith('/') || norm.includes(':') || norm.split('/').includes('..')) {
          errors.push(`Unsafe path (zip-slip): ${e.name}`);
          continue;
        }
        if (norm.split('/')[0].toLowerCase() === '__macosx') continue;
        fileCount++;
        totalBytes += e.size;
        const base = basename(norm);
        if (!isAllowedMember(base)) errors.push(`Unsupported file type in archive: ${e.name}`);
        const eext = extname(base).toLowerCase();
        if (eext === '.html' || eext === '.htm') htmlFiles.push(norm);
      }
    }
  }

  if (fileCount > MAX_ARCHIVE_FILES) errors.push(`File count (${fileCount}) exceeds limit (${MAX_ARCHIVE_FILES}).`);
  if (totalBytes > MAX_ARCHIVE_BYTES) errors.push(`Total size (${(totalBytes / 1048576).toFixed(1)} MB) exceeds 150 MB limit.`);
  if (htmlFiles.length === 0) errors.push('No HTML entry page (.html/.htm) found.');

  console.log(`DiyarDesk Theme SDK Validator: ${target}`);
  console.log(`  Files: ${fileCount} | Size: ${(totalBytes / 1024).toFixed(1)} KB | HTML entries: ${htmlFiles.length}`);
  for (const w of warnings) console.log(`  [WARN] ${w}`);
  for (const e of errors) console.error(`  [ERROR] ${e}`);
  if (errors.length === 0) {
    console.log('  [OK] Package is valid for DiyarDesk import.');
    process.exit(0);
  }
  process.exit(1);
}

const valIdx = process.argv.indexOf('--validate');
if (valIdx >= 0) {
  const targetArg = process.argv[valIdx + 1];
  if (!targetArg) {
    console.error('Usage: node prototype/serve.mjs --validate <dir|.zip|.deskland>');
    process.exit(2);
  }
  validateTarget(targetArg).catch(err => {
    console.error(`[ERROR] ${err.message}`);
    process.exit(1);
  });
} else {
  const host = '127.0.0.1';
  const port = Number(process.env.PORT) || 5178;

  createServer(async (req, res) => {
    let path;
    try {
      path = decodeURIComponent(new URL(req.url, 'http://127.0.0.1').pathname);
    } catch {
      res.writeHead(400).end('bad request');
      return;
    }
    if (path === '/') path = '/flat.html';
    const file = normalize(join(root, path));
    if (file !== root && !file.startsWith(rootPrefix)) {
      res.writeHead(403).end();
      return;
    }
    try {
      const body = await readFile(file);
      res.writeHead(200, {
        'Content-Type': types[extname(file).toLowerCase()] || 'application/octet-stream',
        'Cache-Control': 'no-store',
        'Content-Security-Policy': csp,
        'X-Content-Type-Options': 'nosniff',
        'Referrer-Policy': 'no-referrer',
      });
      res.end(body);
    } catch {
      res.writeHead(404).end('not found');
    }
  }).listen(port, host, () => {
    console.log(`Settings UI:   http://${host}:${port}/settings.html`);
    console.log(`SDK Simulator: http://${host}:${port}/sdk-simulator.html`);
  });
}
