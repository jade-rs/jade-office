// DOM offsets are UTF-16; the Rust adapter validates and converts them.
export function replacement(before, after) {
  let start = 0;
  while (start < before.length && start < after.length && before[start] === after[start]) start++;
  if (start > 0 && /[\uDC00-\uDFFF]/.test(before[start] ?? after[start] ?? "")) start--;
  let end = before.length, tail = after.length;
  while (end > start && tail > start && before[end - 1] === after[tail - 1]) { end--; tail--; }
  if (end < before.length && /[\uDC00-\uDFFF]/.test(before[end])) { end++; tail++; }
  return { start, end, text: after.slice(start, tail) };
}

export function anchor(chars, offset) {
  let units = 0, left = null;
  for (const ch of chars) {
    if (units >= offset) return { left, right: ch.id };
    units += ch.units;
    left = ch.id;
  }
  return { left, right: null };
}

export function position(chars, point) {
  let units = 0, afterLeft = null;
  for (const ch of chars) {
    if (ch.id === point.right) return units;
    units += ch.units;
    if (ch.id === point.left) afterLeft = units;
  }
  return afterLeft ?? (point.left === null ? 0 : units);
}

// Changing a textarea's value also changes its selection. Keep that DOM
// boundary in one place for the composition/reconnect regression tests.
export function renderInput(input, previous, next, composing, focused, localCaret) {
  // Even setSelectionRange with an unchanged confirmed model can move the
  // caret out of the uncommitted IME text. The browser owns both until end.
  if (composing) return;
  const points = focused ? [anchor(previous.chars,input.selectionStart),anchor(previous.chars,input.selectionEnd)] : null;
  input.value = next.text;
  if (points) input.setSelectionRange(position(next.chars,points[0]),position(next.chars,points[1]));
  if (localCaret) input.setSelectionRange(localCaret.start,localCaret.end);
}
