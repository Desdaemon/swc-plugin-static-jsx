export function myHtml(raw, ...subs) {
  return String.raw(raw, ...subs.map(sub => JSON.stringify(sub)))
}
