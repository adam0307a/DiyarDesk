'use strict';

// Deskland · Frontline Pocket (战线包围) 2D painter atlas + instanced WebGL2 renderer
// Clean-room grand-strategy division counters (HOI4 / Age of History inspired):
// Turkish, German, and British heraldic crests, NATO unit symbols (Infantry, Armor,
// Artillery, Motorized), offensive attack arrows, supply forts, and artillery flashes.

const POCKET_COLS = 8;
const POCKET_ROWS = 4;
const POCKET_CELL = 128;

let pocketAtlasTex = null;
let pocketInstPR = null;
let pocketInstVAO = null;
let pocketInstBufA = null;
let pocketInstBufB = null;
let pocketInstCap = 0;

function pRoundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + r);
  ctx.lineTo(x + w, y + h - r);
  ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  ctx.lineTo(x + r, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

function pDrawCrescentEmblem(ctx, x, y, r) {
  // Clean-room Turkish crimson shield with crescent and five-pointed star
  ctx.save();
  ctx.translate(x, y);
  ctx.fillStyle = '#c81d25';
  pRoundRect(ctx, -r, -r * 0.85, r * 2, r * 1.7, 3);
  ctx.fill();
  ctx.strokeStyle = '#f8e6b0';
  ctx.lineWidth = 1.4;
  ctx.stroke();

  // White crescent
  ctx.fillStyle = '#ffffff';
  ctx.beginPath();
  ctx.arc(-r * 0.14, 0, r * 0.46, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#c81d25';
  ctx.beginPath();
  ctx.arc(r * 0.04, 0, r * 0.36, 0, Math.PI * 2);
  ctx.fill();

  // Small five-point star
  ctx.fillStyle = '#ffffff';
  ctx.beginPath();
  const sx = r * 0.44, sy = 0, sr = r * 0.22;
  for (let i = 0; i < 10; i++) {
    const rad = i % 2 === 0 ? sr : sr * 0.42;
    const a = (i / 10) * Math.PI * 2 - Math.PI / 2;
    const px = sx + Math.cos(a) * rad;
    const py = sy + Math.sin(a) * rad;
    if (i === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  ctx.closePath();
  ctx.fill();
  ctx.restore();
}

function pDrawGermanCrossEmblem(ctx, x, y, r) {
  // Clean-room historical stylized heraldic Iron Cross / Balkenkreuz emblem
  ctx.save();
  ctx.translate(x, y);
  ctx.fillStyle = '#32383e';
  pRoundRect(ctx, -r, -r * 0.85, r * 2, r * 1.7, 3);
  ctx.fill();
  ctx.strokeStyle = '#d4dbe0';
  ctx.lineWidth = 1.4;
  ctx.stroke();

  // White outer cross rim
  ctx.fillStyle = '#f2f5f8';
  const wOuter = r * 0.42, lenOuter = r * 0.68;
  ctx.fillRect(-lenOuter, -wOuter * 0.5, lenOuter * 2, wOuter);
  ctx.fillRect(-wOuter * 0.5, -lenOuter * 0.85, wOuter, lenOuter * 1.7);

  // Dark inner cross core
  ctx.fillStyle = '#181b1e';
  const wInner = r * 0.24, lenInner = r * 0.52;
  ctx.fillRect(-lenInner, -wInner * 0.5, lenInner * 2, wInner);
  ctx.fillRect(-wInner * 0.5, -lenInner * 0.85, wInner, lenInner * 1.7);
  ctx.restore();
}

function pDrawBritishCrownEmblem(ctx, x, y, r) {
  // Clean-room British royal blue shield with crimson saltire & gold crown
  ctx.save();
  ctx.translate(x, y);
  ctx.fillStyle = '#193b78';
  pRoundRect(ctx, -r, -r * 0.85, r * 2, r * 1.7, 3);
  ctx.fill();

  ctx.save();
  pRoundRect(ctx, -r, -r * 0.85, r * 2, r * 1.7, 3);
  ctx.clip();
  // Diagonal white & crimson cross bands
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 4.2;
  ctx.beginPath();
  ctx.moveTo(-r, -r * 0.85); ctx.lineTo(r, r * 0.85);
  ctx.moveTo(r, -r * 0.85); ctx.lineTo(-r, r * 0.85);
  ctx.stroke();
  ctx.strokeStyle = '#cf242b';
  ctx.lineWidth = 2.4;
  ctx.beginPath();
  ctx.moveTo(-r, 0); ctx.lineTo(r, 0);
  ctx.moveTo(0, -r * 0.85); ctx.lineTo(0, r * 0.85);
  ctx.stroke();
  ctx.restore();

  // Gold heraldic crown centerpiece
  ctx.fillStyle = '#f2c144';
  ctx.beginPath();
  ctx.moveTo(-r * 0.45, r * 0.25);
  ctx.lineTo(-r * 0.52, -r * 0.25);
  ctx.lineTo(-r * 0.18, r * 0.02);
  ctx.lineTo(0, -r * 0.36);
  ctx.lineTo(r * 0.18, r * 0.02);
  ctx.lineTo(r * 0.52, -r * 0.25);
  ctx.lineTo(r * 0.45, r * 0.25);
  ctx.closePath();
  ctx.fill();

  ctx.strokeStyle = '#f2d37a';
  ctx.lineWidth = 1.4;
  pRoundRect(ctx, -r, -r * 0.85, r * 2, r * 1.7, 3);
  ctx.stroke();
  ctx.restore();
}

function pPaintDivisionCard(ctx, ox, oy, faction, unitType, cardStyle) {
  // faction: 0 = tr, 1 = de, 2 = en
  // unitType: 0 = infantry (X), 1 = armor (tank oval), 2 = artillery (dot), 3 = motorized (X + wheels)
  ctx.save();
  ctx.translate(ox + 64, oy + 64);

  const palettes = [
    { top: '#b82128', bodyA: '#e8d8b6', bodyB: '#cbb486', border: '#6b181b', ink: '#261812', natAccent: '#e23a3f' }, // TR
    { top: '#3a4249', bodyA: '#cfd6dc', bodyB: '#9aa4ad', border: '#20262b', ink: '#14181c', natAccent: '#6c7a86' }, // DE
    { top: '#1d4486', bodyA: '#ebe2cc', bodyB: '#cbbd9c', border: '#142a52', ink: '#192234', natAccent: '#3268c2' }, // EN
  ];
  const pal = palettes[faction];

  // Beveled drop shadow
  ctx.fillStyle = 'rgba(14, 16, 18, 0.48)';
  pRoundRect(ctx, -44 + 4, -29 + 6, 88, 58, 7);
  ctx.fill();

  // Main Age-of-History style counter card frame
  const grad = ctx.createLinearGradient(0, -29, 0, 29);
  grad.addColorStop(0, pal.bodyA);
  grad.addColorStop(1, pal.bodyB);
  ctx.fillStyle = grad;
  pRoundRect(ctx, -44, -29, 88, 58, 7);
  ctx.fill();

  // Top faction banner header strip
  ctx.save();
  pRoundRect(ctx, -44, -29, 88, 58, 7);
  ctx.clip();
  ctx.fillStyle = pal.top;
  ctx.fillRect(-44, -29, 88, 17);
  ctx.fillStyle = 'rgba(255,255,255,0.16)';
  ctx.fillRect(-44, -29, 88, 4);
  ctx.restore();

  // Card outer metallic border + inner bevel
  ctx.strokeStyle = pal.border;
  ctx.lineWidth = 2.6;
  pRoundRect(ctx, -44, -29, 88, 58, 7);
  ctx.stroke();

  // Nation emblem inside left side of banner/card
  if (faction === 0) pDrawCrescentEmblem(ctx, -27, -14, 12);
  else if (faction === 1) pDrawGermanCrossEmblem(ctx, -27, -14, 12);
  else pDrawBritishCrownEmblem(ctx, -27, -14, 12);

  // Division size marker (XX) in top header bar
  ctx.fillStyle = '#f8f6ee';
  ctx.font = '800 10px "Consolas", monospace';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(cardStyle ? 'XXX' : 'XX', 14, -20);

  // Inner NATO tactical box right-of-center
  const bx = 14, by = 4, bw = 42, bh = 25;
  ctx.fillStyle = 'rgba(255, 252, 244, 0.72)';
  pRoundRect(ctx, bx - bw * 0.5, by - bh * 0.5, bw, bh, 2.5);
  ctx.fill();
  ctx.strokeStyle = pal.ink;
  ctx.lineWidth = 2.0;
  ctx.stroke();

  // NATO unit symbol geometry
  ctx.strokeStyle = pal.ink;
  ctx.fillStyle = pal.ink;
  ctx.lineWidth = 2.1;
  if (unitType === 0) {
    // Infantry crossed rifles
    ctx.beginPath();
    ctx.moveTo(bx - bw * 0.45, by - bh * 0.45);
    ctx.lineTo(bx + bw * 0.45, by + bh * 0.45);
    ctx.moveTo(bx + bw * 0.45, by - bh * 0.45);
    ctx.lineTo(bx - bw * 0.45, by + bh * 0.45);
    ctx.stroke();
  } else if (unitType === 1) {
    // Armored / Panzer tank tread loop
    pRoundRect(ctx, bx - 13, by - 6.5, 26, 13, 6.5);
    ctx.stroke();
  } else if (unitType === 2) {
    // Artillery center solid shell circle + flank lines
    ctx.beginPath();
    ctx.arc(bx, by, 5.0, 0, Math.PI * 2);
    ctx.fill();
  } else {
    // Mechanized / Motorized (Infantry X + tank loop)
    ctx.beginPath();
    ctx.moveTo(bx - bw * 0.45, by - bh * 0.45);
    ctx.lineTo(bx + bw * 0.45, by + bh * 0.45);
    ctx.moveTo(bx + bw * 0.45, by - bh * 0.45);
    ctx.lineTo(bx - bw * 0.45, by + bh * 0.45);
    ctx.stroke();
    pRoundRect(ctx, bx - 11, by - 5.5, 22, 11, 5.5);
    ctx.stroke();
  }

  // Strength & Organization status bars along left/bottom edge (classic HOI4 touch!)
  ctx.fillStyle = '#222629';
  ctx.fillRect(-39, 20, 78, 4.5);
  ctx.fillStyle = '#4ad164'; // Org bar
  ctx.fillRect(-38.5, 20.5, 42, 3.5);
  ctx.fillStyle = '#e5a93b'; // Strength bar
  ctx.fillRect(5.5, 20.5, 32, 3.5);

  // Subtle veteran regiment pip beside emblem
  ctx.fillStyle = pal.natAccent;
  ctx.beginPath();
  ctx.arc(-27, 8, 4.2, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#fff5d4';
  ctx.lineWidth = 1.1;
  ctx.stroke();

  ctx.restore();
}

function pPaintEnemyHQMarker(ctx, ox, oy) {
  // Encircled pocket center marker: warning red/dark diamond shield
  ctx.save();
  ctx.translate(ox + 64, oy + 64);

  ctx.fillStyle = 'rgba(20, 10, 10, 0.42)';
  ctx.beginPath();
  ctx.arc(3, 4, 24, 0, Math.PI * 2);
  ctx.fill();

  const g = ctx.createRadialGradient(-5, -5, 2, 0, 0, 22);
  g.addColorStop(0, '#dc3b32');
  g.addColorStop(1, '#7a1916');
  ctx.fillStyle = g;
  ctx.beginPath();
  ctx.arc(0, 0, 21, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#ffd7a8';
  ctx.lineWidth = 2.4;
  ctx.stroke();

  // Crossed defensive swords / pocket exclamation motif
  ctx.strokeStyle = '#fff6e8';
  ctx.lineWidth = 2.6;
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.moveTo(-9, -9); ctx.lineTo(9, 9);
  ctx.moveTo(9, -9); ctx.lineTo(-9, 9);
  ctx.stroke();
  ctx.restore();
}

function pPaintAttackArrow(ctx, ox, oy, factionColor) {
  // Clean curved/straight Age of History thrust arrow pointing along +X
  ctx.save();
  ctx.translate(ox + 64, oy + 64);

  ctx.fillStyle = 'rgba(12, 16, 18, 0.35)';
  ctx.beginPath();
  ctx.moveTo(-26, -6 + 3);
  ctx.lineTo(6, -6 + 3);
  ctx.lineTo(6, -15 + 3);
  ctx.lineTo(30, 3);
  ctx.lineTo(6, 15 + 3);
  ctx.lineTo(6, 6 + 3);
  ctx.lineTo(-26, 6 + 3);
  ctx.closePath();
  ctx.fill();

  const g = ctx.createLinearGradient(-26, 0, 30, 0);
  g.addColorStop(0, factionColor[0]);
  g.addColorStop(1, factionColor[1]);
  ctx.fillStyle = g;
  ctx.beginPath();
  ctx.moveTo(-26, -6);
  ctx.lineTo(6, -6);
  ctx.lineTo(6, -15);
  ctx.lineTo(30, 0);
  ctx.lineTo(6, 15);
  ctx.lineTo(6, 6);
  ctx.lineTo(-26, 6);
  ctx.closePath();
  ctx.fill();
  ctx.strokeStyle = '#fff5dc';
  ctx.lineWidth = 1.6;
  ctx.stroke();
  ctx.restore();
}

function pPaintFortDepot(ctx, ox, oy) {
  ctx.save();
  ctx.translate(ox + 64, oy + 64);
  ctx.fillStyle = 'rgba(18, 20, 18, 0.38)';
  ctx.beginPath();
  ctx.arc(3, 4, 21, 0, Math.PI * 2);
  ctx.fill();

  // Star fortress bastion polygon
  ctx.fillStyle = '#5e6b58';
  ctx.strokeStyle = '#252c22';
  ctx.lineWidth = 2.0;
  ctx.beginPath();
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2;
    const rad = i % 2 === 0 ? 21 : 13;
    const x = Math.cos(a) * rad, y = Math.sin(a) * rad;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.closePath();
  ctx.fill();
  ctx.stroke();

  ctx.fillStyle = '#e7d8b0';
  ctx.beginPath();
  ctx.arc(0, 0, 6.5, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#33402e';
  ctx.beginPath();
  ctx.arc(0, 0, 2.8, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function pPaintArtilleryFlash(ctx, ox, oy) {
  ctx.save();
  ctx.translate(ox + 64, oy + 64);
  const g = ctx.createRadialGradient(0, 0, 2, 0, 0, 34);
  g.addColorStop(0, 'rgba(255, 250, 210, 0.96)');
  g.addColorStop(0.35, 'rgba(255, 166, 48, 0.72)');
  g.addColorStop(0.7, 'rgba(220, 74, 24, 0.25)');
  g.addColorStop(1, 'rgba(220, 74, 24, 0)');
  ctx.fillStyle = g;
  ctx.beginPath();
  ctx.arc(0, 0, 34, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function pPaintBattleSmoke(ctx, ox, oy) {
  ctx.save();
  ctx.translate(ox + 64, oy + 64);
  for (const [bx, by, r, c] of [
    [0, 0, 30, 'rgba(72, 68, 64, 0.34)'],
    [-12, -8, 22, 'rgba(130, 122, 114, 0.26)'],
    [13, 6, 20, 'rgba(92, 88, 84, 0.28)'],
  ]) {
    const g = ctx.createRadialGradient(bx, by, 2, bx, by, r);
    g.addColorStop(0, c);
    g.addColorStop(1, 'rgba(80, 80, 80, 0)');
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.arc(bx, by, r, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();
}

function buildPocketAtlasCanvas() {
  const cvAtlas = document.createElement('canvas');
  cvAtlas.width = POCKET_COLS * POCKET_CELL;
  cvAtlas.height = POCKET_ROWS * POCKET_CELL;
  const ctx = cvAtlas.getContext('2d');

  // Row 0..2: Division counters for Turkish (0), German (1), British (2)
  // Columns: 0 Infantry, 1 Armor, 2 Artillery, 3 Motorized, 4..7 Elite/Corp variant (XXX)
  for (let faction = 0; faction < 3; faction++) {
    for (let u = 0; u < 4; u++) {
      pPaintDivisionCard(ctx, u * POCKET_CELL, faction * POCKET_CELL, faction, u, 0);
      pPaintDivisionCard(ctx, (u + 4) * POCKET_CELL, faction * POCKET_CELL, faction, u, 1);
    }
  }

  // Row 3: Pocket tactical props & arrows
  pPaintEnemyHQMarker(ctx, 0 * POCKET_CELL, 3 * POCKET_CELL);
  pPaintAttackArrow(ctx, 1 * POCKET_CELL, 3 * POCKET_CELL, ['#c8232c', '#ff6454']); // TR Red arrow
  pPaintAttackArrow(ctx, 2 * POCKET_CELL, 3 * POCKET_CELL, ['#3f4952', '#a8bac8']); // DE Steel arrow
  pPaintAttackArrow(ctx, 3 * POCKET_CELL, 3 * POCKET_CELL, ['#1e4c9a', '#5ca6ff']); // EN Royal arrow
  pPaintFortDepot(ctx, 4 * POCKET_CELL, 3 * POCKET_CELL);
  pPaintArtilleryFlash(ctx, 5 * POCKET_CELL, 3 * POCKET_CELL);
  pPaintBattleSmoke(ctx, 6 * POCKET_CELL, 3 * POCKET_CELL);

  return cvAtlas;
}

const POCKET_INST_VS = `#version 300 es
layout(location = 0) in vec2 aQuad;
layout(location = 1) in vec2 aPos;
layout(location = 2) in float aSize;
layout(location = 3) in float aRot;
layout(location = 4) in vec2 aCell;
layout(location = 5) in float aAlpha;

uniform vec2 uRes;
uniform vec2 uGridAtlas;

out vec2 vUv;
out float vAlpha;

void main() {
  float c = cos(aRot);
  float s = sin(aRot);
  vec2 rot = vec2(aQuad.x * c - aQuad.y * s, aQuad.x * s + aQuad.y * c) * aSize;
  vec2 wp = aPos + rot;
  vec2 ndc = vec2(wp.x / uRes.x * 2.0 - 1.0, 1.0 - wp.y / uRes.y * 2.0);
  gl_Position = vec4(ndc, 0.0, 1.0);
  vUv = (aCell + (aQuad + 0.5)) / uGridAtlas;
  vAlpha = aAlpha;
}`;

const POCKET_INST_FS = `#version 300 es
precision highp float;
in vec2 vUv;
in float vAlpha;
uniform sampler2D uAtlas;
out vec4 o;
void main() {
  vec4 t = texture(uAtlas, vUv);
  if (t.a * vAlpha < 0.01) discard;
  o = vec4(t.rgb, t.a * vAlpha);
}`;

function initPocketAtlas() {
  if (pocketAtlasTex || !gl) return;
  const atlasCanvas = buildPocketAtlasCanvas();
  pocketAtlasTex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, pocketAtlasTex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, atlasCanvas);
  gl.generateMipmap(gl.TEXTURE_2D);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  pocketInstPR = program(POCKET_INST_VS, POCKET_INST_FS);
  pocketInstVAO = gl.createVertexArray();
  gl.bindVertexArray(pocketInstVAO);

  const quad = new Float32Array([
    -0.5, -0.5, 0.5, -0.5, -0.5, 0.5,
    0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
  ]);
  pocketInstBufA = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, pocketInstBufA);
  gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

  pocketInstBufB = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, pocketInstBufB);
  const STRIDE = 7 * 4;
  const attrs = [
    [1, 2, 0],
    [2, 1, 8],
    [3, 1, 12],
    [4, 2, 16],
    [5, 1, 24],
  ];
  for (const [loc, cnt, off] of attrs) {
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, cnt, gl.FLOAT, false, STRIDE, off);
    gl.vertexAttribDivisor(loc, 1);
  }
  gl.bindVertexArray(null);
}

/* ======================================================================
   文件夹语言识别 & 包围圈军团逻辑
   - faction 0: 土耳其部队 (TR)
   - faction 1: 德意志部队 (DE)
   - faction 2: 英伦部队 (EN)
   ====================================================================== */
const TR_STRONG = new Set([
  'tr', 'tur', 'turk', 'turkiye', 'turkce', 'turkish', 'ankara', 'istanbul', 'izmir', 'diyar',
  'diyarbakir', 'diyardesk', 'bursa', 'antalya', 'adana', 'konya', 'gaziantep', 'trabzon',
  'samsun', 'eskisehir', 'kayseri', 'mersin', 'canakkale', 'sakarya', 'osmanli', 'anadolu',
  'cumhuriyet', 'vatan', 'ataturk', 'ordu', 'tugay', 'tumen', 'cephe', 'sancak', 'bayrak',
  'hilal', 'bozkurt', 'mehmetcik', 'karargah', 'komutan', 'komutanlik', 'zafer',
]);
const DE_STRONG = new Set([
  'de', 'deu', 'ger', 'deutsch', 'deutschland', 'german', 'germany', 'almanca', 'almanya',
  'alman', 'berlin', 'munchen', 'muenchen', 'munich', 'hamburg', 'koln', 'koeln', 'cologne',
  'frankfurt', 'stuttgart', 'dresden', 'leipzig', 'nurnberg', 'nuernberg', 'hannover',
  'dusseldorf', 'duesseldorf', 'bremen', 'wien', 'vienna', 'zurich', 'zuerich', 'bayern',
  'bavaria', 'preussen', 'prussia', 'rhein', 'ruhr', 'panzer', 'heer', 'wehrmacht', 'luftwaffe',
  'kriegsmarine', 'kompanie', 'kaserne', 'reich', 'kaiser', 'armee', 'bunker', 'festung',
  'divisionen', 'feldzug', 'hauptquartier', 'ostfront', 'westfront', 'blitzkrieg', 'truppen',
]);
const EN_STRONG = new Set([
  'en', 'eng', 'uk', 'gb', 'gbr', 'us', 'usa', 'english', 'britain', 'british', 'england',
  'ingilizce', 'ingiltere', 'ingiliz', 'london', 'manchester', 'liverpool', 'birmingham',
  'oxford', 'cambridge', 'york', 'edinburgh', 'scotland', 'wales', 'commonwealth', 'royal',
  'crown', 'empire', 'navy', 'raf', 'brigade', 'regiment', 'battalion', 'allies', 'allied',
  'headquarters', 'hq', 'churchill', 'montgomery', 'spitfire', 'lancaster', 'dunkirk',
  'normandy',
]);

const TR_WORDS = new Set([
  'belge', 'belgeler', 'belgelerim', 'belgesi', 'indirilen', 'indirilenler', 'masaustu',
  'resim', 'resimler', 'resimlerim', 'muzik', 'muzikler', 'videolar', 'oyun', 'oyunlar',
  'proje', 'projeler', 'projesi', 'yedek', 'yedekler', 'yedegi', 'yeni', 'klasor', 'klasorler',
  'klasoru', 'not', 'notlar', 'notlarim', 'notu', 'odev', 'odevler', 'arsiv', 'arsivler',
  'dosya', 'dosyalar', 'dosyasi', 'rapor', 'raporlar', 'raporu', 'fatura', 'faturalar',
  'fotograf', 'fotograflar', 'calisma', 'calismalar', 'calismalarim', 'goruntu', 'goruntuler',
  'goruntusu', 'ekran', 'kurulum', 'tasarim', 'tasarimlar', 'gunluk', 'hesap', 'hesaplar',
  'liste', 'listeler', 'listesi', 'deneme', 'denemeler', 'cop', 'kutusu', 'kutu', 'sozlesme',
  'ozgecmis', 'gider', 'giderler', 'gelir', 'sunum', 'sunumlar', 'bilgisayar', 'araclar',
  'arac', 'onemli', 'kitap', 'kitaplar', 'kitaplik', 'ders', 'dersler', 'planlama', 'yazilim',
  'hatirlatma', 'tablo', 'tablolar', 'sinav', 'sinavlar', 'butce', 'kisisel', 'ayar', 'ayarlar',
  'sifre', 'sifreler', 'duvar', 'kagidi', 'tema', 'temalar', 'simge', 'simgeler', 'ikon',
  'ikonlar', 'merhaba', 'tatil', 'egitim', 'muhasebe', 'vergi', 'haber', 'haberler', 'iletisim',
  'takvim', 'gorev', 'gorevler', 'aile', 'okul', 'sinif', 'ogrenci', 'ogretmen', 'sarki',
  'sarkilar', 'filmler', 'sohbet', 'kullanici', 'sistem', 'harita', 'haritalar', 'pusula',
  'kale', 'gemi', 'ucak', 'deniz', 'dag', 'orman', 'bulut', 'yagmur', 'gunes', 'yildiz',
  'sehir', 'kopru', 'bahce', 'kedi', 'kopek', 'kahve', 'defter', 'kalem', 'masa', 'asker',
  'savas', 'baris', 'topcu', 'piyade', 'zirhli', 'subay', 'albay', 'baslangic', 'bitis',
  'sonuc', 'ozet', 'kayit', 'kayitlar', 'veri', 'veriler', 'dokuman', 'dokumanlar', 'indir',
  'yukle', 'guncelleme', 'acil', 'gizli', 'ozel', 'genel', 'ortak', 'eski', 'son', 'ilk',
  'hizli', 'kolay', 'guvenlik', 'ag', 'uygulama', 'uygulamalar', 'programlar', 'icerik',
  'makale', 'yazi', 'yazilar', 'ceviri', 'sozluk', 'rehber', 'kilavuz', 'destek', 'yardim',
  'ev', 'is', 'isler', 'evrak', 'evraklar', 'dilekce', 'tutanak', 'baski', 'tarama',
]);
const DE_WORDS = new Set([
  'ordner', 'dokument', 'dokumente', 'bilder', 'bild', 'musik', 'arbeitsplatz', 'papierkorb',
  'herunterladen', 'entwurf', 'entwurfe', 'entwuerfe', 'projekt', 'projekte', 'bericht',
  'berichte', 'ubersicht', 'uebersicht', 'buro', 'buero', 'rechnung', 'rechnungen', 'vertrag',
  'vertrage', 'vertraege', 'sicherung', 'notizen', 'notiz', 'zusammenfassung', 'schule',
  'arbeit', 'arbeiten', 'datei', 'dateien', 'karte', 'karten', 'spiel', 'spiele', 'foto',
  'fotos', 'neu', 'neue', 'neuer', 'neues', 'wichtig', 'festung', 'plane', 'plaene', 'ablage',
  'studium', 'urlaub', 'schrift', 'schriften', 'daten', 'werkzeug', 'werkzeuge', 'strasse',
  'strassen', 'haus', 'hauser', 'haeuser', 'stadt', 'buch', 'bucher', 'buecher', 'brief',
  'briefe', 'aufgabe', 'aufgaben', 'prufung', 'pruefung', 'ubung', 'uebung', 'bewerbung',
  'lebenslauf', 'steuer', 'steuern', 'einstellungen', 'programm', 'programme', 'hilfe',
  'tabelle', 'tabellen', 'vorlage', 'vorlagen', 'termin', 'termine', 'nachricht', 'nachrichten',
  'passwort', 'passworter', 'sicherheit', 'netzwerk', 'hallo', 'mein', 'meine', 'meiner',
  'und', 'der', 'die', 'das', 'fur', 'fuer', 'uber', 'ueber', 'mit', 'von', 'aus', 'nach',
  'bei', 'zum', 'zur', 'verzeichnis', 'bildschirm', 'bildschirmfoto', 'hintergrund', 'systeme',
  'anwendung', 'anwendungen', 'benutzer', 'privat', 'geheim', 'familie', 'freunde', 'reise',
  'reisen', 'kalender', 'einkauf', 'haushalt', 'finanzen', 'konto', 'kasse', 'listen',
  'protokoll', 'handbuch', 'anleitung', 'quelle', 'quellen', 'beispiel', 'beispiele', 'losung',
  'loesung', 'ergebnis', 'ergebnisse', 'geschichte', 'krieg', 'frieden', 'soldat', 'soldaten',
  'angriff', 'verteidigung', 'befehl', 'befehle', 'lager', 'stutzpunkt', 'stuetzpunkt',
  'grenze', 'flotte', 'geschwader', 'infanterie', 'artillerie', 'aufklarung', 'aufklaerung',
  'nachschub', 'versorgung', 'einsatz', 'stellung', 'frontlinie', 'wald', 'berg', 'fluss',
  'meer', 'himmel', 'wetter', 'sonne', 'mond', 'stern', 'sommer', 'herbst', 'fruhling',
  'fruehling', 'klasse', 'lehrer', 'schuler', 'schueler', 'universitat', 'universitaet',
  'vorlesung', 'klausur', 'hausarbeit', 'forschung', 'entwicklung', 'zeitung', 'magazin',
  'filme', 'spiegel', 'zeit', 'tag', 'jahr', 'monat', 'woche', 'heute', 'morgen', 'gestern',
]);
const EN_WORDS = new Set([
  'documents', 'document', 'downloads', 'download', 'desktop', 'pictures', 'picture', 'photos',
  'photo', 'music', 'videos', 'video', 'projects', 'project', 'screenshots', 'screenshot',
  'backup', 'backups', 'archive', 'archives', 'games', 'game', 'notes', 'note', 'report',
  'reports', 'invoice', 'invoices', 'draft', 'drafts', 'resume', 'work', 'study', 'tools',
  'tool', 'folder', 'folders', 'new', 'setup', 'config', 'configs', 'test', 'tests', 'code',
  'server', 'steam', 'home', 'trash', 'bin', 'readme', 'apps', 'app', 'media', 'assets',
  'files', 'file', 'movies', 'movie', 'images', 'image', 'plot', 'misc', 'application',
  'applications', 'final', 'settings', 'options', 'manual', 'guide', 'help', 'support',
  'course', 'courses', 'homework', 'exam', 'exams', 'book', 'books', 'budget', 'tax', 'taxes',
  'family', 'travel', 'wallpaper', 'wallpapers', 'theme', 'themes', 'icons', 'icon', 'scripts',
  'script', 'build', 'builds', 'release', 'releases', 'install', 'installer', 'compiler',
  'controller', 'handler', 'calendar', 'tasks', 'task', 'todo', 'ideas', 'idea', 'design',
  'designs', 'hello', 'welcome', 'my', 'the', 'and', 'for', 'with', 'from', 'private',
  'personal', 'public', 'shared', 'secret', 'important', 'urgent', 'old', 'latest', 'quick',
  'fast', 'security', 'password', 'passwords', 'network', 'system', 'user', 'users', 'account',
  'accounts', 'finance', 'receipt', 'receipts', 'contract', 'contracts', 'letter', 'letters',
  'summary', 'overview', 'presentation', 'slides', 'spreadsheet', 'sheet', 'table', 'template',
  'templates', 'schedule', 'meeting', 'meetings', 'message', 'messages', 'mail', 'email',
  'inbox', 'library', 'reading', 'research', 'paper', 'papers', 'thesis', 'lecture', 'lectures',
  'school', 'college', 'university', 'class', 'student', 'teacher', 'office', 'company',
  'business', 'client', 'clients', 'sales', 'marketing', 'product', 'products', 'source',
  'sources', 'data', 'database', 'logs', 'log', 'cache', 'temp', 'tmp', 'export', 'exports',
  'import', 'imports', 'update', 'updates', 'patch', 'patches', 'mod', 'mods', 'map', 'maps',
  'battle', 'frontline', 'siege', 'fortress', 'army', 'infantry', 'armor', 'armour',
  'artillery', 'division', 'divisions', 'corps', 'fleet', 'squadron', 'command', 'orders',
  'supply', 'recon', 'defense', 'defence', 'attack', 'victory', 'peace', 'war', 'soldier',
  'soldiers', 'general', 'officer', 'camp', 'base', 'border', 'coast', 'island', 'forest',
  'mountain', 'river', 'ocean', 'weather', 'summer', 'winter', 'spring', 'autumn', 'solar',
  'modular', 'trailer', 'popular', 'regular', 'dollar',
]);

function foldToAsciiLower(s) {
  let out = '';
  for (const c of s) {
    if (c === 'İ' || c === 'I' || c === 'ı') out += 'i';
    else if (c === 'ç' || c === 'Ç') out += 'c';
    else if (c === 'ğ' || c === 'Ğ') out += 'g';
    else if (c === 'ş' || c === 'Ş') out += 's';
    else if (c === 'ö' || c === 'Ö') out += 'o';
    else if (c === 'ü' || c === 'Ü') out += 'u';
    else if (c === 'ä' || c === 'Ä') out += 'a';
    else if (c === 'ß' || c === 'ẞ') out += 'ss';
    else out += c.toLowerCase();
  }
  return out;
}

function splitCamelAndSepTokens(stem) {
  const isUpper = ch => ch !== ch.toLowerCase() && ch === ch.toUpperCase();
  const isLower = ch => ch !== ch.toUpperCase() && ch === ch.toLowerCase();
  const isSep = ch => /[\s_\-.()[\]/\\:&+,'0-9]/.test(ch);
  const chars = Array.from(stem);
  const tokens = [];
  let cur = '';
  for (let i = 0; i < chars.length; i++) {
    const c = chars[i];
    if (isSep(c)) {
      if (cur) { tokens.push(foldToAsciiLower(cur)); cur = ''; }
      continue;
    }
    if (cur && isUpper(c)) {
      const prev = chars[i - 1];
      const next = chars[i + 1];
      if (isLower(prev) || (isUpper(prev) && next && isLower(next))) {
        tokens.push(foldToAsciiLower(cur));
        cur = '';
      }
    }
    cur += c;
  }
  if (cur) tokens.push(foldToAsciiLower(cur));
  return tokens;
}

function detectFolderNation(rawName, salt) {
  const mode = Math.round(P.faction || 0);
  if (mode === 1) return 0; // Turkish forced
  if (mode === 2) return 1; // German forced
  if (mode === 3) return 2; // British forced

  const raw = String(rawName || '').trim();
  if (!raw) return 2;
  const stem = raw.replace(/\.[a-z0-9]{1,5}$/i, '');
  const foldedWhole = foldToAsciiLower(stem);
  const tokens = splitCamelAndSepTokens(stem);

  let sTR = 0, sDE = 0, sEN = 0;

  // 1. Character / diacritic cues on original stem
  const trUniqueCnt = (stem.match(/[ğĞşŞıİ]/g) || []).length;
  if (trUniqueCnt > 0) sTR += 8.5 + Math.min(2, trUniqueCnt - 1) * 1.5;
  if (/[çÇ]/.test(stem)) sTR += 6.0;
  const deUniqueCnt = (stem.match(/[ßẞäÄ]/g) || []).length;
  if (deUniqueCnt > 0) sDE += 8.5 + Math.min(2, deUniqueCnt - 1) * 1.5;
  if (/[öÖüÜ]/.test(stem)) { sTR += 2.4; sDE += 2.4; }
  if (/[wWqQxX]/.test(stem)) { sDE += 0.6; sEN += 0.6; }

  // 2. Token-level strong cues, dictionaries, morphology & digraphs
  for (const tok of tokens) {
    const strongTR = TR_STRONG.has(tok);
    const strongDE = DE_STRONG.has(tok);
    const strongEN = EN_STRONG.has(tok);
    if (strongTR) sTR += 11.0;
    if (strongDE) sDE += 11.0;
    if (strongEN) sEN += 11.0;

    const wordTR = TR_WORDS.has(tok);
    const wordDE = DE_WORDS.has(tok);
    const wordEN = EN_WORDS.has(tok);
    if (wordTR) sTR += 6.0;
    if (wordDE) sDE += 6.0;
    if (wordEN) sEN += 6.0;

    if (!strongDE && (tok.startsWith('almany') || tok.startsWith('alman'))) sDE += 11.0;
    if (!strongEN && (tok.startsWith('ingiliz') || tok.startsWith('ingilter'))) sEN += 11.0;
    if (!strongTR && (tok.startsWith('turkiy') || tok.startsWith('turkce') || tok.startsWith('turkler'))) sTR += 11.0;

    const len = tok.length;
    const anyTR = strongTR || wordTR;
    const anyDE = strongDE || wordDE;
    const anyEN = strongEN || wordEN;

    if (len >= 4 && !anyDE && !anyEN) {
      if (/(lari|leri|larim|lerim|likler|liklar|lukler|iyor|ecek|acak|mesi|masi|deki|daki|sizlik|cilik|culuk)$/.test(tok)) {
        sTR += 3.0;
      } else {
        const isEnDeLarLer = /(ular|ilar|olar|ellar|ller|dler|iler|tler|hler|bler|pler)$/.test(tok);
        if (
          (!isEnDeLarLer && /(lar|ler)$/.test(tok)) ||
          /(lik|luk|cim|sim|yor|mis|mus)$/.test(tok) ||
          (len >= 5 && /(mek|mak|siz|suz|cik|cuk|dir|dur|tir|tur)$/.test(tok))
        ) {
          sTR += 2.4;
        }
      }
    }

    if (len >= 4 && !anyTR && !anyEN) {
      if (
        /(ung|ungen|heit|keit|schaft|chen|lein|isch|lich|tsch|platz|stadt|strasse|zeug|karte|lager|ieren|iert)$/.test(tok) ||
        (len >= 5 && /(werk|haus|buch|berg|burg|dorf|amt)$/.test(tok))
      ) {
        sDE += 3.2;
      }
      if (tok.includes('sch')) sDE += 3.0;
      else if (/(tz|pf|chs)/.test(tok)) sDE += 2.0;
      if (len >= 5 && /(ae|oe|ue)/.test(tok)) sDE += 1.5;
    }

    if (len >= 4 && !anyTR && !anyDE) {
      if (/(tion|sion|ment|ness|ing|shot|app|able|ible|ful|less|ware|load|sync|ship|ward|ize|ify|ous|ive)$/.test(tok)) {
        sEN += 2.6;
      }
      if (/(th|wh|sh|gh|ph|qu|oo|ee|igh|ould)/.test(tok)) {
        sEN += 1.6;
      }
    }
  }

  // 3. Compound substring hints inside glued lowercase names
  let subTR = 0, subDE = 0, subEN = 0;
  for (const w of TR_STRONG) if (w.length >= 4 && foldedWhole.includes(w)) subTR = Math.min(9.0, subTR + 3.0);
  for (const w of TR_WORDS) if (w.length >= 4 && foldedWhole.includes(w)) subTR = Math.min(9.0, subTR + 3.0);
  for (const w of DE_STRONG) if (w.length >= 4 && foldedWhole.includes(w)) subDE = Math.min(9.0, subDE + 3.0);
  for (const w of DE_WORDS) if (w.length >= 4 && foldedWhole.includes(w)) subDE = Math.min(9.0, subDE + 3.0);
  for (const w of EN_STRONG) if (w.length >= 4 && foldedWhole.includes(w)) subEN = Math.min(9.0, subEN + 3.0);
  for (const w of EN_WORDS) if (w.length >= 4 && foldedWhole.includes(w)) subEN = Math.min(9.0, subEN + 3.0);
  sTR += subTR;
  sDE += subDE;
  sEN += subEN;

  if (sTR > sDE && sTR > sEN && sTR >= 2) return 0;
  if (sDE > sTR && sDE > sEN && sDE >= 2) return 1;
  if (sEN > 0) return 2;
  if (sTR >= 2 && sTR === sDE) return (((salt | 0) % 2) + 2) % 2 === 0 ? 0 : 1;

  return /[^\x00-\x7f]/.test(stem) ? ((((salt | 0) % 3) + 3) % 3) : 2;
}

let forts = [];
let trees = forts; // share.js settle alias
let divisions = [];
let pocketHQ = [];
let skirmishes = [];

function pocketHash(a, b, k) {
  let h = Math.imul((a | 0) + 17, 374761393) ^ Math.imul((b | 0) + 31, 668265263) ^ Math.imul((seed | 0) + (k | 0) * 23, 2246822519);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

function pocketNearestIcon(px, py) {
  if (typeof icons === 'undefined' || !icons.length) return { ic: null, dist: 999 };
  let bestIc = null;
  let bestD = 999;
  for (const ic of icons) {
    const cx = ic.x + ic.w * 0.5;
    const cy = ic.y + ic.h * 0.5;
    const d = Math.hypot(px - cx, py - cy);
    if (d < bestD) { bestD = d; bestIc = ic; }
  }
  return { ic: bestIc, dist: bestD };
}

function pocketGradient(px, py) {
  const gx = landS(px + 6, py) - landS(px - 6, py);
  const gy = landS(px, py + 6) - landS(px, py - 6);
  const len = Math.max(0.001, Math.hypot(gx, gy));
  // Points inward toward lowest SDF (into icon pocket)
  return { x: -gx / len, y: -gy / len };
}

function rebuildPocketProps() {
  if (!W || !H) return;
  forts = [];
  trees = forts;

  // Place friendly strategic star-fort depots in outer free countryside
  const cell = 175;
  const cols = Math.ceil(W / cell);
  const rows = Math.ceil((H - 48) / cell);
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      if (pocketHash(c, r, 11) > 0.58) continue;
      const x = (c + 0.25 + 0.5 * pocketHash(c, r, 12)) * cell;
      const y = (r + 0.25 + 0.5 * pocketHash(c, r, 13)) * cell;
      if (x < 40 || x > W - 40 || y < 40 || y > H - 76) continue;
      const ok = landS(x, y) > 48 ? 1 : 0;
      forts.push({ x, y, grow: ok, want: ok, rot: pocketHash(c, r, 14) * Math.PI * 2 });
    }
  }

  syncPocketForces(true);
}

function syncPocketForces(resetAll) {
  const wantDivs = Math.max(0, Math.min(24, Math.round(P.divisions)));
  for (const f of forts) f.want = landS(f.x, f.y) > 48 ? 1 : 0;

  // Identify encircled pocket clusters & place pocket HQ warning marker on major clusters
  pocketHQ = [];
  const iconClusterId = new Array(typeof icons !== 'undefined' ? icons.length : 0).fill(0);
  const clusters = [];
  if (typeof icons !== 'undefined' && icons.length && P.adaptive) {
    const visited = new Array(icons.length).fill(false);
    const linkDist = Math.max(120, (P.sigma || 78) * 2.25);
    for (let i = 0; i < icons.length; i++) {
      if (visited[i]) continue;
      const q = [i];
      visited[i] = true;
      const cid = clusters.length;
      const cluster = [];
      while (q.length) {
        const cur = q.shift();
        iconClusterId[cur] = cid;
        cluster.push(icons[cur]);
        const cx = icons[cur].x + icons[cur].w * 0.5;
        const cy = icons[cur].y + icons[cur].h * 0.5;
        for (let j = 0; j < icons.length; j++) {
          if (visited[j]) continue;
          const jx = icons[j].x + icons[j].w * 0.5;
          const jy = icons[j].y + icons[j].h * 0.5;
          if (Math.hypot(cx - jx, cy - jy) < linkDist) {
            visited[j] = true;
            q.push(j);
          }
        }
      }
      let sx = 0, sy = 0;
      for (const ic of cluster) { sx += ic.x + ic.w * 0.5; sy += ic.y + ic.h * 0.5; }
      const cObj = { x: sx / cluster.length, y: sy / cluster.length, count: cluster.length };
      clusters.push(cObj);
      if (cluster.length >= 2) pocketHQ.push(cObj);
    }
  }

  // Collect candidate frontline encirclement slots grouped per pocket cluster
  const clusterBuckets = clusters.map(() => []);
  const stepScan = 22;
  for (let py = 36; py < H - 72; py += stepScan) {
    for (let px = 44; px < W - 44; px += stepScan) {
      const s = landS(px, py);
      if (s < 18 || s > 48) continue;
      let bestIdx = -1, bestD = 1e9;
      if (typeof icons !== 'undefined') {
        for (let i = 0; i < icons.length; i++) {
          const d = Math.hypot(px - (icons[i].x + icons[i].w * 0.5), py - (icons[i].y + icons[i].h * 0.5));
          if (d < bestD) { bestD = d; bestIdx = i; }
        }
      }
      if (bestIdx < 0) continue;
      const cid = iconClusterId[bestIdx];
      const cl = clusters[cid];
      const nation = detectFolderNation(icons[bestIdx].name, Math.round(px + py));
      const grad = pocketGradient(px, py);
      clusterBuckets[cid].push({
        x: px, y: py,
        inX: grad.x, inY: grad.y,
        nation,
        ang: Math.atan2(py - cl.y, px - cl.x),
        distS: Math.abs(s - 30),
      });
    }
  }

  const chosenSlots = [];
  const activeBuckets = clusterBuckets.filter(b => b.length > 0);
  if (!activeBuckets.length) {
    for (let i = 0; i < wantDivs; i++) {
      const a = (i / Math.max(1, wantDivs)) * Math.PI * 2;
      const px = W * 0.5 + Math.cos(a) * Math.min(W, H) * 0.28;
      const py = H * 0.48 + Math.sin(a) * Math.min(W, H) * 0.24;
      const forcedMode = Math.round(P.faction || 0);
      const nat = forcedMode > 0 ? forcedMode - 1 : (i % 3);
      chosenSlots.push({ x: px, y: py, inX: -Math.cos(a), inY: -Math.sin(a), nation: nat });
    }
  } else {
    // Select best slot per angular sector inside each pocket cluster, then round-robin evenly across pockets
    const SECTOR_COUNT = 16;
    const perClusterOrdered = activeBuckets.map(bucket => {
      const bestPerSector = new Array(SECTOR_COUNT).fill(null);
      for (const slot of bucket) {
        const sec = Math.floor(((slot.ang + Math.PI) / (Math.PI * 2)) * SECTOR_COUNT) % SECTOR_COUNT;
        if (!bestPerSector[sec] || slot.distS < bestPerSector[sec].distS) bestPerSector[sec] = slot;
      }
      const clean = bestPerSector.filter(Boolean);
      return clean.length ? clean : bucket.slice(0, SECTOR_COUNT);
    });

    // Distribute wantDivs symmetrically around each pocket cluster in turn
    const perClusterShare = perClusterOrdered.map((_, idx) => {
      const base = Math.floor(wantDivs / perClusterOrdered.length);
      return base + (idx < (wantDivs % perClusterOrdered.length) ? 1 : 0);
    });
    for (let c = 0; c < perClusterOrdered.length; c++) {
      const list = perClusterOrdered[c];
      const wantHere = perClusterShare[c];
      for (let k = 0; k < wantHere; k++) {
        const posIdx = Math.floor((k * list.length) / Math.max(1, wantHere)) % list.length;
        chosenSlots.push(list[posIdx]);
      }
    }
  }

  for (let i = 0; i < wantDivs; i++) {
    const slot = chosenSlots[i];
    const unitType = Math.floor(pocketHash(i, 71, 51) * 4);
    const elite = pocketHash(i, 72, 52) < 0.35 ? 4 : 0;
    if (i < divisions.length) {
      const d = divisions[i];
      d.tx = slot.x;
      d.ty = slot.y;
      d.inX = slot.inX;
      d.inY = slot.inY;
      d.nation = slot.nation;
    } else {
      divisions.push({
        idx: i,
        x: slot.x, y: slot.y,
        tx: slot.x, ty: slot.y,
        inX: slot.inX, inY: slot.inY,
        nation: slot.nation,
        unitCol: unitType + elite,
        phase: pocketHash(i, 73, 53) * Math.PI * 2,
        fireCooldown: pocketHash(i, 74, 54) * 3.5,
      });
    }
  }
}

function updatePocketTargets(snap) {
  for (const f of forts) {
    const ok = landS(f.x, f.y) > 48 ? 1 : 0;
    f.want = ok;
    if (snap) f.grow = ok;
  }
  syncPocketForces(snap);
}

function stepPocketProps(dt, timeSec) {
  const rate = dt * 2.8;
  for (const f of forts) {
    if (f.grow < f.want) f.grow = Math.min(f.want, f.grow + rate);
    else if (f.grow > f.want) f.grow = Math.max(f.want, f.grow - rate * 1.5);
  }

  const pace = Math.max(0.1, P.pace || 0);
  // Divisions march smoothly toward updated pocket encirclement targets & fire artillery inward
  for (const d of divisions) {
    const dx = d.tx - d.x;
    const dy = d.ty - d.y;
    const dist = Math.hypot(dx, dy);
    if (dist > 1) {
      const spd = Math.min(dist * 4.5, (52 + dist * 0.85) * pace) * dt;
      d.x += (dx / dist) * Math.min(dist, spd);
      d.y += (dy / dist) * Math.min(dist, spd);
    }

    if ((P.skirmish || 0) > 0.04) {
      d.fireCooldown -= dt * (0.5 + pace * 0.9);
      if (d.fireCooldown <= 0) {
        d.fireCooldown = 1.8 + pocketHash(d.idx, Math.floor(timeSec * 2), 81) * 3.6;
        const muzzleX = d.x + d.inX * 26;
        const muzzleY = d.y + d.inY * 26;
        skirmishes.push({
          x: muzzleX, y: muzzleY,
          vx: d.inX * 14, vy: d.inY * 14,
          age: 0, dur: 0.42,
          kind: pocketHash(d.idx, Math.round(muzzleX), 82) < 0.55 ? 5 : 6,
          size: 28 + pocketHash(d.idx, 3, 83) * 18,
        });
      }
    }
  }

  for (let i = skirmishes.length - 1; i >= 0; i--) {
    const sk = skirmishes[i];
    sk.age += dt;
    sk.x += sk.vx * dt;
    sk.y += sk.vy * dt;
    if (sk.age >= sk.dur) skirmishes.splice(i, 1);
  }
}

function drawPocketProps(timeSec) {
  if (!gl) return;
  initPocketAtlas();

  const items = [];
  const pace = P.pace || 0;

  // 1. Friendly star-fort depots in outer map
  if ((P.supply || 0) > 0.05) {
    for (const f of forts) {
      if (f.grow <= 0.01) continue;
      const ease = f.grow * f.grow * (3 - 2 * f.grow);
      items.push({
        sortY: f.y - 200,
        x: f.x, y: f.y,
        size: 36 * (0.3 + 0.7 * ease),
        rot: f.rot * 0.2,
        col: 4, row: 3,
        alpha: Math.min(1, ease) * (0.45 + 0.55 * P.supply),
      });
    }
  }

  // 2. Pocket center garrison HQ warning badges on multi-icon pockets
  for (const hq of pocketHQ) {
    const pulse = 1.0 + 0.08 * Math.sin(timeSec * 3.2);
    items.push({
      sortY: hq.y - 100,
      x: hq.x, y: hq.y,
      size: 38 * pulse,
      rot: 0,
      col: 0, row: 3,
      alpha: 0.78,
    });
  }

  // 3. Attack thrust arrows + Age of History / HOI4 Division counters sorted along Y
  const ordered = divisions.slice().sort((a, b) => a.y - b.y);
  for (const d of ordered) {
    const ang = Math.atan2(d.inY, d.inX);
    const marchStep = Math.sin(timeSec * (1.8 + 1.4 * pace) + d.phase) * 3.5;

    if ((P.arrows || 0) > 0.05) {
      const arrowX = d.x + d.inX * (27 + marchStep);
      const arrowY = d.y + d.inY * (27 + marchStep);
      items.push({
        x: arrowX, y: arrowY,
        size: 30,
        rot: ang,
        col: 1 + d.nation, row: 3,
        alpha: Math.min(1, 0.3 + 0.7 * P.arrows),
      });
    }

    // Division counter card stays upright with slight recoil lunge along attack vector
    const sway = Math.sin(timeSec * (1.2 + pace) + d.phase) * 0.03;
    items.push({
      x: d.x + d.inX * (marchStep * 0.45),
      y: d.y + d.inY * (marchStep * 0.45),
      size: 58,
      rot: sway,
      col: d.unitCol, row: d.nation,
      alpha: 0.98,
    });
  }

  // 4. Artillery flashes & tactical battle smoke
  if ((P.skirmish || 0) > 0.04) {
    for (const sk of skirmishes) {
      const t = sk.age / sk.dur;
      items.push({
        x: sk.x, y: sk.y,
        size: sk.size * (sk.kind === 5 ? (0.65 + t * 0.6) : (0.7 + t * 1.1)),
        rot: sk.age * 1.8,
        col: sk.kind, row: 3,
        alpha: (1 - t) * (0.4 + 0.6 * P.skirmish),
      });
    }
  }

  if (!items.length) return;
  const data = new Float32Array(items.length * 7);
  for (let i = 0; i < items.length; i++) {
    const it = items[i];
    const o = i * 7;
    data[o] = it.x;
    data[o + 1] = it.y;
    data[o + 2] = it.size;
    data[o + 3] = it.rot;
    data[o + 4] = it.col;
    data[o + 5] = it.row;
    data[o + 6] = it.alpha;
  }

  gl.enable(gl.BLEND);
  gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ZERO, gl.ONE);
  gl.useProgram(pocketInstPR.p);
  gl.bindVertexArray(pocketInstVAO);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, pocketAtlasTex);
  gl.uniform1i(pocketInstPR.U.uAtlas, 0);
  gl.uniform2f(pocketInstPR.U.uRes, cv.width / cv._scale, cv.height / cv._scale);
  gl.uniform2f(pocketInstPR.U.uGridAtlas, POCKET_COLS, POCKET_ROWS);

  gl.bindBuffer(gl.ARRAY_BUFFER, pocketInstBufB);
  if (items.length > pocketInstCap) {
    pocketInstCap = Math.max(items.length, 64);
    gl.bufferData(gl.ARRAY_BUFFER, pocketInstCap * 28, gl.DYNAMIC_DRAW);
  }
  gl.bufferSubData(gl.ARRAY_BUFFER, 0, data);
  gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, items.length);
  gl.bindVertexArray(null);
  gl.disable(gl.BLEND);
}
