// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure pointer long-press controller for mobile hold-to-open menus.

export const LONG_PRESS_MS = 450;
export const LONG_PRESS_MOVE_PX = 10;
export const PRIMARY_BUTTON = 0;

export interface LongPressStart {
  button: number;
  clientX: number;
  clientY: number;
}

export interface LongPressMove {
  clientX: number;
  clientY: number;
}

export interface LongPressController {
  start: (event: LongPressStart) => void;
  move: (event: LongPressMove) => void;
  cancel: () => void;
  consumeSuppressedClick: () => boolean;
}

export function createLongPressController(
  onTrigger: (x: number, y: number) => void,
  holdMs = LONG_PRESS_MS,
): LongPressController {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let origin: { x: number; y: number } | null = null;
  let suppressNextClick = false;

  const cancel = (): void => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    origin = null;
  };

  const start = (event: LongPressStart): void => {
    if (event.button !== PRIMARY_BUTTON) {
      return;
    }
    cancel();
    suppressNextClick = false;
    const { clientX, clientY } = event;
    origin = { x: clientX, y: clientY };
    timer = setTimeout(() => {
      timer = null;
      origin = null;
      suppressNextClick = true;
      onTrigger(clientX, clientY);
    }, holdMs);
  };

  const move = (event: LongPressMove): void => {
    if (origin === null) {
      return;
    }
    const dx = Math.abs(event.clientX - origin.x);
    const dy = Math.abs(event.clientY - origin.y);
    if (dx > LONG_PRESS_MOVE_PX || dy > LONG_PRESS_MOVE_PX) {
      cancel();
    }
  };

  const consumeSuppressedClick = (): boolean => {
    if (!suppressNextClick) {
      return false;
    }
    suppressNextClick = false;
    return true;
  };

  return { start, move, cancel, consumeSuppressedClick };
}
