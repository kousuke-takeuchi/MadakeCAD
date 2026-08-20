// ワールド座標(用紙mm)⇔スクリーン座標(px)の変換。
// screen = world * scale + origin

export interface Pt {
  x: number;
  y: number;
}

/** グリッド/ピンピッチ (mm)。 */
export const GRID_PITCH = 2.5;

const MIN_SCALE = 0.05;
const MAX_SCALE = 200;

export class Viewport {
  /** 1mmあたりのpx数。 */
  scale = 4;
  originX = 0;
  originY = 0;

  toScreen(p: Pt): Pt {
    return { x: p.x * this.scale + this.originX, y: p.y * this.scale + this.originY };
  }

  toWorld(p: Pt): Pt {
    return { x: (p.x - this.originX) / this.scale, y: (p.y - this.originY) / this.scale };
  }

  /** スクリーン上のanchor点を固定したままズームする。 */
  zoomAt(anchor: Pt, factor: number): void {
    const w = this.toWorld(anchor);
    this.scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, this.scale * factor));
    this.originX = anchor.x - w.x * this.scale;
    this.originY = anchor.y - w.y * this.scale;
  }

  /** スクリーンpx単位でパンする。 */
  pan(dxPx: number, dyPx: number): void {
    this.originX += dxPx;
    this.originY += dyPx;
  }

  /** ワールド座標をグリッドに丸める。 */
  snap(p: Pt, pitch: number = GRID_PITCH): Pt {
    return {
      x: Math.round(p.x / pitch) * pitch,
      y: Math.round(p.y / pitch) * pitch,
    };
  }

  /** 用紙(w×h mm)がキャンバス(px)に収まるようにフィットさせる。 */
  fit(paperW: number, paperH: number, canvasW: number, canvasH: number, marginPx = 40): void {
    const sx = (canvasW - marginPx * 2) / paperW;
    const sy = (canvasH - marginPx * 2) / paperH;
    this.scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, Math.min(sx, sy)));
    this.originX = (canvasW - paperW * this.scale) / 2;
    this.originY = (canvasH - paperH * this.scale) / 2;
  }
}
