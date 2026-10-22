/**
 * OpenVegas Pro 21 — Human Face Demo Thumbnail Renderer for Video FX, Frei0r & CorridorKey
 * Draws a detailed human portrait (head, natural skin tones, eyes with catchlights, eyebrows,
 * nose, lips, hair, and studio backdrop) and applies the actual pixel/canvas transformation
 * of each effect so every FX card displays a live visual demonstration on a human face!
 */

export function renderHumanFaceEffectThumbnail(
  canvas: HTMLCanvasElement,
  effectType: string
): void {
  const w = canvas.width || 68;
  const h = canvas.height || 50;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return;

  // Offscreen buffer for base human portrait
  const off = document.createElement('canvas');
  off.width = w;
  off.height = h;
  const octx = off.getContext('2d', { willReadFrequently: true })!;

  // 1. Studio Backdrop (Bright Green Screen for CorridorKey, or Studio Gradient for others)
  if (effectType === 'corridorKey') {
    // Split backdrop: Left side raw green screen, Right side unmixed studio dark cyber backdrop!
    octx.fillStyle = '#00d828';
    octx.fillRect(0, 0, w / 2, h);
    const rightGrad = octx.createLinearGradient(w / 2, 0, w, h);
    rightGrad.addColorStop(0, '#0f172a');
    rightGrad.addColorStop(1, '#1e3a8a');
    octx.fillStyle = rightGrad;
    octx.fillRect(w / 2, 0, w / 2, h);
  } else {
    const bgGrad = octx.createLinearGradient(0, 0, w, h);
    bgGrad.addColorStop(0, '#1e293b');
    bgGrad.addColorStop(0.6, '#334155');
    bgGrad.addColorStop(1, '#0f172a');
    octx.fillStyle = bgGrad;
    octx.fillRect(0, 0, w, h);
  }

  const cx = w / 2;
  const cy = h * 0.48;

  // 2. Shoulders / Collar
  octx.fillStyle = '#1e3a5f';
  octx.beginPath();
  octx.ellipse(cx, h + 4, 24, 14, 0, Math.PI, 0, false);
  octx.fill();

  // 3. Neck
  octx.fillStyle = '#d49b78';
  octx.fillRect(cx - 5, cy + 10, 10, 11);

  // 4. Back Hair Silhouette
  octx.fillStyle = '#271c18';
  octx.beginPath();
  octx.ellipse(cx, cy - 3, 15.5, 17.5, 0, 0, Math.PI * 2);
  octx.fill();

  // 5. Ears
  octx.fillStyle = '#dba382';
  octx.beginPath();
  octx.ellipse(cx - 13.5, cy + 1, 2.5, 4.5, 0, 0, Math.PI * 2);
  octx.ellipse(cx + 13.5, cy + 1, 2.5, 4.5, 0, 0, Math.PI * 2);
  octx.fill();

  // 6. Human Face Oval with 3D Studio Lighting Gradient
  const skinGrad = octx.createRadialGradient(cx - 3, cy - 4, 2, cx, cy, 16);
  skinGrad.addColorStop(0, '#f6ccb2');
  skinGrad.addColorStop(0.65, '#e2a887');
  skinGrad.addColorStop(1, '#b87b5c');
  octx.fillStyle = skinGrad;
  octx.beginPath();
  octx.ellipse(cx, cy, 13, 16, 0, 0, Math.PI * 2);
  octx.fill();

  // Cheek warmth
  octx.fillStyle = 'rgba(225, 105, 95, 0.24)';
  octx.beginPath();
  octx.arc(cx - 6.5, cy + 3, 3.5, 0, Math.PI * 2);
  octx.arc(cx + 6.5, cy + 3, 3.5, 0, Math.PI * 2);
  octx.fill();

  // 7. Front Hair / Bangs
  octx.fillStyle = '#231815';
  octx.beginPath();
  octx.ellipse(cx, cy - 11.5, 13.5, 6.5, 0, Math.PI, 0, false);
  octx.fill();
  octx.beginPath();
  octx.moveTo(cx - 13, cy - 8);
  octx.quadraticCurveTo(cx - 4, cy - 15, cx + 13, cy - 7);
  octx.lineTo(cx + 13, cy - 14);
  octx.lineTo(cx - 13, cy - 14);
  octx.fill();

  // 8. Eyebrows
  octx.strokeStyle = '#36241d';
  octx.lineWidth = 1.4;
  octx.beginPath();
  octx.moveTo(cx - 8.5, cy - 4.2);
  octx.quadraticCurveTo(cx - 5.2, cy - 5.8, cx - 2.2, cy - 4.4);
  octx.moveTo(cx + 2.2, cy - 4.4);
  octx.quadraticCurveTo(cx + 5.2, cy - 5.8, cx + 8.5, cy - 4.2);
  octx.stroke();

  // 9. Eyes (Sclera + Iris + Pupil + Catchlight)
  const drawEye = (ex: number, ey: number) => {
    octx.fillStyle = '#f8fafc';
    octx.beginPath();
    octx.ellipse(ex, ey, 3.1, 1.8, 0, 0, Math.PI * 2);
    octx.fill();

    octx.fillStyle = '#2563eb';
    octx.beginPath();
    octx.arc(ex, ey, 1.5, 0, Math.PI * 2);
    octx.fill();

    octx.fillStyle = '#0f172a';
    octx.beginPath();
    octx.arc(ex, ey, 0.8, 0, Math.PI * 2);
    octx.fill();

    octx.fillStyle = '#ffffff';
    octx.fillRect(ex - 0.8, ey - 1.1, 0.9, 0.9);
  };
  drawEye(cx - 5.2, cy - 1.5);
  drawEye(cx + 5.2, cy - 1.5);

  // 10. Nose bridge & tip shadow
  octx.strokeStyle = 'rgba(138, 82, 58, 0.65)';
  octx.lineWidth = 1.1;
  octx.beginPath();
  octx.moveTo(cx - 0.5, cy - 2);
  octx.lineTo(cx - 1.3, cy + 4);
  octx.lineTo(cx + 1.2, cy + 4.4);
  octx.stroke();

  // 11. Lips
  octx.fillStyle = '#c45a55';
  octx.beginPath();
  octx.ellipse(cx, cy + 8.5, 4.5, 1.9, 0, 0, Math.PI * 2);
  octx.fill();
  octx.strokeStyle = '#8c3532';
  octx.lineWidth = 0.8;
  octx.beginPath();
  octx.moveTo(cx - 4.3, cy + 8.5);
  octx.lineTo(cx + 4.3, cy + 8.5);
  octx.stroke();

  // ─── APPLY SPECIFIC VIDEO FX / FREI0R / CORRIDORKEY TO THE PORTRAIT ───
  ctx.clearRect(0, 0, w, h);
  ctx.save();

  if (effectType === 'brightnessContrast') {
    ctx.filter = 'brightness(118%) contrast(132%) saturate(118%)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'colorGrade' || effectType === 'frei0r.coloradj_RGB') {
    ctx.filter = 'contrast(114%) saturate(135%)';
    ctx.drawImage(off, 0, 0);
    // Teal shadows / Orange skin highlight split
    const grad = ctx.createLinearGradient(0, 0, w, h);
    grad.addColorStop(0, 'rgba(6, 182, 212, 0.28)');
    grad.addColorStop(1, 'rgba(249, 115, 22, 0.28)');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, w, h);
  } else if (effectType === 'blur') {
    ctx.filter = 'blur(2.4px)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'sepia') {
    ctx.filter = 'sepia(95%) contrast(108%)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'invert') {
    ctx.filter = 'invert(100%)';
    ctx.drawImage(off, 0, 0);
  } else if (effectType === 'vignette') {
    ctx.drawImage(off, 0, 0);
    const vgrad = ctx.createRadialGradient(cx, cy, 8, cx, cy, w * 0.55);
    vgrad.addColorStop(0, 'rgba(0,0,0,0)');
    vgrad.addColorStop(1, 'rgba(0,0,0,0.82)');
    ctx.fillStyle = vgrad;
    ctx.fillRect(0, 0, w, h);
  } else if (effectType === 'vhsGlitch' || effectType === 'frei0r.distort0r') {
    // Draw RGB chromatic shift + scanlines
    ctx.drawImage(off, -2, 0);
    ctx.globalAlpha = 0.5;
    ctx.drawImage(off, 2, 0);
    ctx.globalAlpha = 1.0;
    ctx.fillStyle = 'rgba(0,0,0,0.28)';
    for (let y = 0; y < h; y += 3) {
      ctx.fillRect(0, y, w, 1);
    }
  } else if (effectType === 'frei0r.pixeliz0r') {
    const small = document.createElement('canvas');
    small.width = 12;
    small.height = 9;
    const sctx = small.getContext('2d')!;
    sctx.drawImage(off, 0, 0, 12, 9);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(small, 0, 0, w, h);
  } else if (effectType === 'frei0r.glow') {
    ctx.drawImage(off, 0, 0);
    ctx.save();
    ctx.filter = 'blur(3.5px) brightness(140%)';
    ctx.globalCompositeOperation = 'screen';
    ctx.globalAlpha = 0.75;
    ctx.drawImage(off, 0, 0);
    ctx.restore();
  } else if (effectType === 'frei0r.cartoon' || effectType === 'frei0r.posterize') {
    ctx.drawImage(off, 0, 0);
    const img = ctx.getImageData(0, 0, w, h);
    const d = img.data;
    const levels = effectType === 'frei0r.cartoon' ? 4 : 3;
    const step = 255 / levels;
    for (let i = 0; i < d.length; i += 4) {
      d[i] = Math.round(d[i] / step) * step;
      d[i + 1] = Math.round(d[i + 1] / step) * step;
      d[i + 2] = Math.round(d[i + 2] / step) * step;
    }
    ctx.putImageData(img, 0, 0);
  } else if (effectType === 'frei0r.sobel') {
    const src = octx.getImageData(0, 0, w, h).data;
    const out = ctx.createImageData(w, h);
    const dst = out.data;
    for (let y = 1; y < h - 1; y++) {
      for (let x = 1; x < w - 1; x++) {
        const idx = (y * w + x) * 4;
        const gx = Math.abs(src[idx + 4] - src[idx - 4]);
        const gy = Math.abs(src[idx + w * 4] - src[idx - w * 4]);
        const mag = Math.min(255, (gx + gy) * 2.4);
        dst[idx] = mag * 0.4;
        dst[idx + 1] = mag;
        dst[idx + 2] = Math.min(255, mag * 1.2);
        dst[idx + 3] = 255;
      }
    }
    ctx.putImageData(out, 0, 0);
  } else if (effectType === 'corridorKey') {
    ctx.drawImage(off, 0, 0);
    // Split line showing Green Screen -> Clean Keyed Alpha Unmix
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(w / 2, 0);
    ctx.lineTo(w / 2, h);
    ctx.stroke();
  } else {
    ctx.drawImage(off, 0, 0);
  }

  ctx.restore();
}
