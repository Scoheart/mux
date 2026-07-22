/** Return the first visible letter or number for a compact resource avatar. */
export function resourceInitial(value: string, fallback = "?"): string {
  const initial = Array.from(value.trim()).find((character) => /[\p{L}\p{N}]/u.test(character));
  return initial?.toLocaleUpperCase() ?? fallback;
}
