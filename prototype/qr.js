// Zero-dependency offline QR Code SVG generator (ISO/IEC 18004, Byte mode, ECC-L, Versions 1-6).
// Exposes window.desklandQrSvg(text, sizePx) for local LAN pairing without external libraries.
(function (root) {
  'use strict';

  var GF_EXP = new Uint8Array(512);
  var GF_LOG = new Uint8Array(256);
  (function initGalois() {
    var x = 1;
    for (var i = 0; i < 255; i++) {
      GF_EXP[i] = x;
      GF_LOG[x] = i;
      x <<= 1;
      if (x & 0x100) x ^= 0x11d;
    }
    for (var j = 255; j < 512; j++) GF_EXP[j] = GF_EXP[j - 255];
  })();

  function gfMul(a, b) {
    if (a === 0 || b === 0) return 0;
    return GF_EXP[GF_LOG[a] + GF_LOG[b]];
  }

  function rsGenerator(degree) {
    var poly = [1];
    for (var i = 0; i < degree; i++) {
      var next = new Array(poly.length + 1);
      for (var k = 0; k < next.length; k++) next[k] = 0;
      var rootVal = GF_EXP[i];
      for (var j = 0; j < poly.length; j++) {
        next[j] ^= poly[j];
        next[j + 1] ^= gfMul(poly[j], rootVal);
      }
      poly = next;
    }
    return poly;
  }

  function rsCompute(data, ecCount) {
    var gen = rsGenerator(ecCount);
    var rem = new Uint8Array(ecCount);
    for (var i = 0; i < data.length; i++) {
      var factor = data[i] ^ rem[0];
      for (var j = 0; j < ecCount - 1; j++) rem[j] = rem[j + 1];
      rem[ecCount - 1] = 0;
      for (var k = 0; k < ecCount; k++) rem[k] ^= gfMul(gen[k + 1], factor);
    }
    return rem;
  }

  // [version, totalDataCodewords, ecCodewordsPerBlock, numBlocks, alignPos[]] (ECC Level L)
  var VERSIONS = [
    null,
    [1, 19, 7, 1, []],
    [2, 34, 10, 1, [6, 18]],
    [3, 55, 15, 1, [6, 22]],
    [4, 80, 20, 1, [6, 26]],
    [5, 108, 26, 1, [6, 30]],
    [6, 136, 18, 2, [6, 34]],
  ];

  function utf8Bytes(str) {
    if (typeof TextEncoder !== 'undefined') return Array.from(new TextEncoder().encode(str));
    var out = [];
    for (var i = 0; i < str.length; i++) {
      var c = str.charCodeAt(i);
      if (c < 0x80) out.push(c);
      else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f));
      else out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f));
    }
    return out;
  }

  function buildCodewords(bytes) {
    var ver = null;
    for (var v = 1; v < VERSIONS.length; v++) {
      if (bytes.length + 2 <= VERSIONS[v][1]) { ver = VERSIONS[v]; break; }
    }
    if (!ver) throw new Error('QR payload too long');
    var dataCap = ver[1], ecPerBlock = ver[2], blocks = ver[3];
    var bits = [];
    function pushBits(val, len) {
      for (var i = len - 1; i >= 0; i--) bits.push((val >>> i) & 1);
    }
    pushBits(0x4, 4); // Byte mode
    pushBits(bytes.length, 8);
    for (var i = 0; i < bytes.length; i++) pushBits(bytes[i], 8);
    var maxBits = dataCap * 8;
    for (var t = 0; t < 4 && bits.length < maxBits; t++) bits.push(0);
    while (bits.length % 8 !== 0) bits.push(0);
    var codewords = [];
    for (var b = 0; b < bits.length; b += 8) {
      var byte = 0;
      for (var k = 0; k < 8; k++) byte = (byte << 1) | bits[b + k];
      codewords.push(byte);
    }
    var pad = [0xec, 0x11], pi = 0;
    while (codewords.length < dataCap) codewords.push(pad[pi++ & 1]);

    var blockSize = dataCap / blocks;
    var dataBlocks = [], ecBlocks = [];
    for (var bi = 0; bi < blocks; bi++) {
      var slice = codewords.slice(bi * blockSize, (bi + 1) * blockSize);
      dataBlocks.push(slice);
      ecBlocks.push(rsCompute(slice, ecPerBlock));
    }
    var finalBytes = [];
    for (var c = 0; c < blockSize; c++) {
      for (var bi2 = 0; bi2 < blocks; bi2++) finalBytes.push(dataBlocks[bi2][c]);
    }
    for (var e = 0; e < ecPerBlock; e++) {
      for (var bi3 = 0; bi3 < blocks; bi3++) finalBytes.push(ecBlocks[bi3][e]);
    }
    return { version: ver[0], align: ver[4], bytes: finalBytes };
  }

  function formatBits(mask) {
    // ECC L = 01
    var data = (1 << 3) | (mask & 7);
    var rem = data << 10;
    for (var i = 14; i >= 10; i--) {
      if ((rem >>> i) & 1) rem ^= 0x537 << (i - 10);
    }
    return ((data << 10) | rem) ^ 0x5412;
  }

  function generateMatrix(text) {
    var payload = buildCodewords(utf8Bytes(String(text || '')));
    var v = payload.version;
    var n = v * 4 + 17;
    var mat = [];
    var isFunc = [];
    for (var y = 0; y < n; y++) {
      mat.push(new Uint8Array(n));
      isFunc.push(new Uint8Array(n));
    }
    function setFunc(x, y, val) {
      if (x >= 0 && y >= 0 && x < n && y < n) {
        mat[y][x] = val ? 1 : 0;
        isFunc[y][x] = 1;
      }
    }
    function drawFinder(cx, cy) {
      for (var dy = -4; dy <= 4; dy++) {
        for (var dx = -4; dx <= 4; dx++) {
          var dist = Math.max(Math.abs(dx), Math.abs(dy));
          setFunc(cx + dx, cy + dy, dist !== 2 && dist !== 4);
        }
      }
    }
    drawFinder(3, 3);
    drawFinder(n - 4, 3);
    drawFinder(3, n - 4);
    for (var i = 0; i < n; i++) {
      if (!isFunc[6][i]) setFunc(i, 6, i % 2 === 0);
      if (!isFunc[i][6]) setFunc(6, i, i % 2 === 0);
    }
    var ap = payload.align;
    for (var ay = 0; ay < ap.length; ay++) {
      for (var ax = 0; ax < ap.length; ax++) {
        var cx = ap[ax], cy = ap[ay];
        if ((cx === 6 && cy === 6) || (cx === 6 && cy === n - 7) || (cx === n - 7 && cy === 6)) continue;
        for (var dy2 = -2; dy2 <= 2; dy2++) {
          for (var dx2 = -2; dx2 <= 2; dx2++) {
            setFunc(cx + dx2, cy + dy2, Math.max(Math.abs(dx2), Math.abs(dy2)) !== 1);
          }
        }
      }
    }
    // Reserve format areas + dark module
    for (var f = 0; f <= 8; f++) {
      if (!isFunc[8][f]) setFunc(f, 8, 0);
      if (!isFunc[f][8]) setFunc(8, f, 0);
    }
    for (var f2 = 0; f2 < 8; f2++) {
      if (!isFunc[8][n - 1 - f2]) setFunc(n - 1 - f2, 8, 0);
      if (!isFunc[n - 1 - f2][8]) setFunc(8, n - 1 - f2, 0);
    }
    setFunc(8, n - 8, 1);

    // Place data bits in zig-zag columns
    var bitIdx = 0;
    var totalBits = payload.bytes.length * 8;
    for (var right = n - 1; right >= 1; right -= 2) {
      if (right === 6) right = 5;
      var upward = ((right + 1) & 2) === 0;
      for (var vert = 0; vert < n; vert++) {
        var row = upward ? n - 1 - vert : vert;
        for (var colOff = 0; colOff < 2; colOff++) {
          var col = right - colOff;
          if (isFunc[row][col]) continue;
          var bit = 0;
          if (bitIdx < totalBits) {
            bit = (payload.bytes[bitIdx >>> 3] >>> (7 - (bitIdx & 7))) & 1;
            bitIdx++;
          }
          // Apply mask pattern 0: (row + col) % 2 === 0
          if (((row + col) & 1) === 0) bit ^= 1;
          mat[row][col] = bit;
        }
      }
    }

    // Write format info for ECC-L + mask 0
    var fb = formatBits(0);
    for (var b0 = 0; b0 <= 5; b0++) mat[8][b0] = (fb >>> (14 - b0)) & 1;
    mat[8][7] = (fb >>> 8) & 1;
    mat[8][8] = (fb >>> 7) & 1;
    mat[7][8] = (fb >>> 6) & 1;
    for (var b1 = 0; b1 <= 5; b1++) mat[5 - b1][8] = (fb >>> (5 - b1)) & 1;
    for (var b2 = 0; b2 < 8; b2++) mat[8][n - 1 - b2] = (fb >>> b2) & 1;
    for (var b3 = 0; b3 < 7; b3++) mat[n - 1 - b3][8] = (fb >>> (14 - b3)) & 1;
    return mat;
  }

  function renderSvg(text, sizePx) {
    var mat = generateMatrix(text);
    var n = mat.length;
    var quiet = 3;
    var dim = n + quiet * 2;
    var size = sizePx || 168;
    var d = '';
    for (var y = 0; y < n; y++) {
      for (var x = 0; x < n; x++) {
        if (mat[y][x]) d += 'M' + (x + quiet) + ',' + (y + quiet) + 'h1v1h-1z';
      }
    }
    return '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ' + dim + ' ' + dim + '" width="' + size + '" height="' + size + '" shape-rendering="crispEdges" role="img" aria-label="LAN Pairing QR Code">'
      + '<rect width="' + dim + '" height="' + dim + '" fill="#ffffff" rx="2"/>'
      + '<path d="' + d + '" fill="#111111"/>'
      + '</svg>';
  }

  root.desklandQrMatrix = generateMatrix;
  root.desklandQrSvg = renderSvg;
})(typeof window !== 'undefined' ? window : globalThis);
