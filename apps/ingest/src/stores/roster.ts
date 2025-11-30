import { defineStore } from "pinia";
import type { Competitor } from "@/types";

// Mock competitor data for V1
const MOCK_COMPETITORS: Competitor[] = [
  {
    id: "1",
    competitorNumber: "1001",
    firstName: "Emma",
    lastName: "Johnson",
    displayName: "1001 Emma J",
    teamName: "Gold Stars",
  },
  {
    id: "2",
    competitorNumber: "1002",
    firstName: "Olivia",
    lastName: "Smith",
    displayName: "1002 Olivia S",
    teamName: "Gold Stars",
  },
  {
    id: "3",
    competitorNumber: "1003",
    firstName: "Ava",
    lastName: "Williams",
    displayName: "1003 Ava W",
    teamName: "Silver Eagles",
  },
  {
    id: "4",
    competitorNumber: "1004",
    firstName: "Sophia",
    lastName: "Brown",
    displayName: "1004 Sophia B",
    teamName: "Silver Eagles",
  },
  {
    id: "5",
    competitorNumber: "1005",
    firstName: "Isabella",
    lastName: "Jones",
    displayName: "1005 Isabella J",
    teamName: "Bronze Bears",
  },
  {
    id: "6",
    competitorNumber: "1006",
    firstName: "Mia",
    lastName: "Garcia",
    displayName: "1006 Mia G",
    teamName: "Bronze Bears",
  },
  {
    id: "7",
    competitorNumber: "1007",
    firstName: "Charlotte",
    lastName: "Miller",
    displayName: "1007 Charlotte M",
    teamName: "Diamond Divas",
  },
  {
    id: "8",
    competitorNumber: "1008",
    firstName: "Amelia",
    lastName: "Davis",
    displayName: "1008 Amelia D",
    teamName: "Diamond Divas",
  },
];

export const useRosterStore = defineStore("roster", {
  state: () => ({
    competitors: MOCK_COMPETITORS,
    isLoading: false,
    error: null as string | null,
  }),

  getters: {
    sortedCompetitors(): Competitor[] {
      return [...this.competitors].sort((a, b) =>
        a.competitorNumber.localeCompare(b.competitorNumber)
      );
    },

    competitorsByTeam(): Record<string, Competitor[]> {
      return this.competitors.reduce(
        (acc, comp) => {
          if (!acc[comp.teamName]) {
            acc[comp.teamName] = [];
          }
          acc[comp.teamName].push(comp);
          return acc;
        },
        {} as Record<string, Competitor[]>
      );
    },
  },

  actions: {
    // Stub for future API integration
    async loadRoster(_eventId: string) {
      this.isLoading = true;
      this.error = null;

      try {
        // TODO: Replace with actual API call
        // const response = await fetch(`/api/events/${eventId}/roster`);
        // this.competitors = await response.json();

        // For now, simulate loading delay
        await new Promise((resolve) => setTimeout(resolve, 300));
        this.competitors = MOCK_COMPETITORS;
      } catch (error) {
        this.error = String(error);
      } finally {
        this.isLoading = false;
      }
    },

    findByNumber(number: string): Competitor | undefined {
      return this.competitors.find((c) => c.competitorNumber === number);
    },

    search(query: string): Competitor[] {
      const lowerQuery = query.toLowerCase();
      return this.competitors.filter(
        (c) =>
          c.competitorNumber.includes(query) ||
          c.firstName.toLowerCase().includes(lowerQuery) ||
          c.lastName.toLowerCase().includes(lowerQuery) ||
          c.teamName.toLowerCase().includes(lowerQuery)
      );
    },
  },
});
