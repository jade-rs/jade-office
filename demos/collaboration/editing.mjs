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
