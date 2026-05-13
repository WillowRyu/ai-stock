import type { AssetKind } from "./ipc";

export function getLocalTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

/** Sensible default timezone for displaying observed_at, given an asset kind. */
export function getDefaultTimezone(kind: AssetKind): string {
  switch (kind) {
    case "us":
      return "America/New_York";
    case "kr":
      return "Asia/Seoul";
    case "crypto":
    case "fx":
    case "com":
    default:
      return getLocalTimezone();
  }
}

export interface TimezoneOption {
  value: string;
  label: string;
}

/** Curated short list of timezones, with the user's detected local timezone first if it is not already present. */
export function getTimezoneOptions(): TimezoneOption[] {
  const local = getLocalTimezone();
  const base: TimezoneOption[] = [
    { value: "America/New_York", label: "미국 동부 (뉴욕)" },
    { value: "America/Los_Angeles", label: "미국 서부 (LA)" },
    { value: "Europe/London", label: "런던" },
    { value: "Asia/Seoul", label: "한국 (서울)" },
    { value: "Asia/Tokyo", label: "일본 (도쿄)" },
    { value: "Asia/Hong_Kong", label: "홍콩" },
    { value: "Asia/Shanghai", label: "상하이" },
    { value: "UTC", label: "UTC" },
  ];
  if (!base.some((o) => o.value === local)) {
    base.unshift({ value: local, label: `현재 위치 (${local})` });
  }
  return base;
}

/**
 * Format an RFC3339 timestamp into a localized string in the given IANA timezone.
 * Uses ko-KR locale formatting (YYYY. MM. DD. HH:MM:SS) since the app's primary
 * audience is Korean. Falls back to the input string if parsing fails.
 */
export function formatObservedAt(iso: string, tz: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  try {
    return new Intl.DateTimeFormat("ko-KR", {
      timeZone: tz,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    }).format(d);
  } catch {
    return iso;
  }
}
