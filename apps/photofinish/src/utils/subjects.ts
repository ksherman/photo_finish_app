import type { SubjectRecord } from "@/types";

export function bibNum(s: Pick<SubjectRecord, "number">): number {
  if (!s.number) return Number.MAX_SAFE_INTEGER;
  const n = parseInt(s.number, 10);
  return Number.isNaN(n) ? Number.MAX_SAFE_INTEGER : n;
}

export function sortSubjectsByNumber<T extends SubjectRecord>(list: T[]): T[] {
  return [...list].sort((a, b) => {
    const diff = bibNum(a) - bibNum(b);
    if (diff !== 0) return diff;
    return a.displayName.localeCompare(b.displayName, undefined, {
      numeric: true,
      sensitivity: "base",
    });
  });
}

/// Instant 0ms client-side search with exact-bib -> bib-prefix -> name-prefix -> substring
/// ranking and strict numeric bib tie-breaking (e.g. 105 before 1000).
export function rankAndFilterSubjects<T extends SubjectRecord>(list: T[], query: string): T[] {
  const raw = query.trim().toLowerCase();
  if (!raw) return sortSubjectsByNumber(list);

  const matches: Array<{ subject: T; rank: number; bib: number }> = [];
  for (const s of list) {
    const num = (s.number || "").toLowerCase();
    const first = s.firstName.toLowerCase();
    const last = (s.lastName || "").toLowerCase();
    const disp = s.displayName.toLowerCase();
    const team = (s.teamName || "").toLowerCase();

    let rank = -1;
    if (num === raw) rank = 0;
    else if (num.startsWith(raw)) rank = 1;
    else if (first.startsWith(raw) || last.startsWith(raw)) rank = 2;
    else if (disp.includes(raw) || team.includes(raw)) rank = 3;

    if (rank >= 0) {
      matches.push({ subject: s, rank, bib: bibNum(s) });
    }
  }

  matches.sort((a, b) => {
    if (a.rank !== b.rank) return a.rank - b.rank;
    if (a.bib !== b.bib) return a.bib - b.bib;
    return a.subject.displayName.localeCompare(b.subject.displayName, undefined, {
      numeric: true,
      sensitivity: "base",
    });
  });

  return matches.map((m) => m.subject);
}
