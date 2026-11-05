import { WIDE_SCREEN, WINDOW } from '../../lib/track';
import type { Key, Rect, Shot } from '../../lib/track';
import type { Scroll } from '../scroll';

interface Shots {
  peek: Shot;
  front: Shot;
  frontSide: Shot;
  iso: Shot;
  isoEnd: Shot;
  status: Shot;
  statusEnd: Shot;
  panes: Shot;
  preview: Shot;
  previewEnd: Shot;
  review: Shot;
  reviewEnd: Shot;
  browser: Shot;
  browserPick: Shot;
  flow: Shot;
  flowEnd: Shot;
  gone: Shot;
  back: Shot;
  shut: Shot;
  away: Shot;
}

const SIDE: Rect = [0, 8, 650, 792];
const PANES: Rect = [316, 8, 964, 792];
const AGENT: Rect = [316, 200, 800, 460];
const DOCK: Rect = [316, 8, 964, 792];
const BROWSER: Rect = [316, 8, 964, 792];
const FLOW: Rect = [420, 8, 860, 792];

function wide(): Shots {
  return {
    peek: { focus: WINDOW, free: 'all', rx: 38, dy: 0.66, zoom: 1.04 },
    front: { focus: WINDOW, free: 'all' },
    frontSide: { focus: WINDOW, free: 'right', zoom: 0.96 },
    iso: { focus: WINDOW, free: 'right', rx: 52, rz: -30, zoom: 1.0, dy: 0.03, dx: -0.01 },
    isoEnd: { focus: WINDOW, free: 'right', rx: 47, rz: -20, zoom: 1.02, dy: 0.03, dx: -0.01 },
    status: { focus: SIDE, free: 'left', ry: 7, rx: 2 },
    statusEnd: { focus: SIDE, free: 'left', ry: -3, rx: -1 },
    panes: { focus: PANES, free: 'right', ry: -4 },
    preview: { focus: AGENT, free: 'left', ry: 5, zoom: 0.94 },
    previewEnd: { focus: AGENT, free: 'left', ry: -2, zoom: 0.98 },
    review: { focus: DOCK, free: 'right', ry: -6 },
    reviewEnd: { focus: DOCK, free: 'right', ry: -2 },
    browser: { focus: BROWSER, free: 'right', ry: -5 },
    browserPick: { focus: BROWSER, free: 'right', ry: -1 },
    flow: { focus: FLOW, free: 'right', ry: -5 },
    flowEnd: { focus: FLOW, free: 'right', ry: 2 },
    gone: { focus: WINDOW, free: 'all', zoom: 0.5, rx: -62, dy: -1.15, fade: 0 },
    back: { focus: WINDOW, free: 'right', zoom: 0.96 },
    shut: { focus: WINDOW, free: 'right', zoom: 0.02, fade: 0 },
    away: { focus: WINDOW, free: 'all', zoom: 1.5, rx: 52, dy: 1.3, fade: 0 },
  };
}

function narrow(): Shots {
  const big = (focus: Rect, zoom = 1): Shot => ({ focus, free: 'all', zoom });
  return {
    peek: { focus: [0, 0, 640, 400], free: 'all', rx: 34, dy: 0.5, dx: 0.34, zoom: 1.0 },
    front: big([0, 0, 800, 800]),
    frontSide: big([0, 0, 720, 500]),
    iso: { focus: WINDOW, free: 'all', rx: 55, rz: -33, zoom: 1.5 },
    isoEnd: { focus: WINDOW, free: 'all', rx: 50, rz: -22, zoom: 1.5 },
    status: big([52, 400, 268, 400]),
    statusEnd: big([52, 400, 268, 400]),
    panes: big([316, 36, 640, 560]),
    preview: big([324, 300, 500, 300]),
    previewEnd: big([324, 300, 500, 300]),
    review: big([852, 36, 428, 500]),
    reviewEnd: big([852, 36, 428, 500]),
    browser: big([632, 36, 648, 760]),
    browserPick: big([632, 36, 648, 760]),
    flow: big([640, 36, 640, 700]),
    flowEnd: big([640, 36, 640, 700]),
    gone: { focus: WINDOW, free: 'all', zoom: 0.6, rx: -62, dy: -1.15, fade: 0 },
    back: { focus: [0, 0, 760, 800], free: 'all', zoom: 0.9, dy: 0.26 },
    shut: { focus: [0, 0, 760, 800], free: 'all', zoom: 0.02, dy: 0.26, fade: 0 },
    away: { focus: WINDOW, free: 'all', zoom: 1.5, rx: 52, dy: 1.3, fade: 0 },
  };
}

export function cameraKeys(scroll: Scroll): Key[] {
  const shots = scroll.width < WIDE_SCREEN ? narrow() : wide();
  const key = (id: string, progress: number, shot: Shot): Key => ({ at: scroll.at(id, progress), shot });
  return [
    key('hero', 0, shots.peek),
    key('hero', 1, shots.front),
    key('layers', 0.05, shots.frontSide),
    key('layers', 0.17, shots.iso),
    key('layers', 0.8, shots.isoEnd),
    key('layers', 0.96, shots.front),
    key('status', 0.07, shots.status),
    key('status', 0.94, shots.statusEnd),
    key('panes', 0.07, shots.panes),
    key('panes', 0.95, shots.panes),
    key('preview', 0.08, shots.preview),
    key('preview', 0.94, shots.previewEnd),
    key('review', 0.08, shots.review),
    key('review', 0.95, shots.reviewEnd),
    key('browser', 0.07, shots.browser),
    key('browser', 0.95, shots.browserPick),
    key('flow', 0.08, shots.flow),
    key('flow', 0.94, shots.flowEnd),
    key('sheets', 0.09, shots.gone),
    key('sheets', 0.95, shots.gone),
    key('close', 0.0, shots.gone),
    key('close', 0.07, shots.back),
    key('close', 0.25, shots.back),
    key('close', 0.34, shots.shut),
    key('close', 0.7, shots.shut),
    key('close', 0.8, shots.back),
    key('close', 0.92, shots.back),
    key('close', 1, shots.away),
  ];
}
