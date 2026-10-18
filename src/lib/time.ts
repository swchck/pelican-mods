/** "5 minutes ago" / "5 минут назад" in `locale`, rounded to the largest whole unit. */
export function relativeTime(then: Date, now: Date, locale: string): string {
  const fmt = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const sec = Math.round((then.getTime() - now.getTime()) / 1000);
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["day", 86400],
    ["hour", 3600],
    ["minute", 60],
  ];
  for (const [unit, size] of units) {
    if (Math.abs(sec) >= size) return fmt.format(Math.round(sec / size), unit);
  }
  return fmt.format(0, "minute");
}
