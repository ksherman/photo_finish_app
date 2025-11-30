import { defineStore } from "pinia";
import type { SessionConfig } from "@/types";

export const useSessionStore = defineStore("session", {
  state: (): SessionConfig => ({
    eventSlug: "",
    photographer: "",
    gym: "",
    sessionNumber: "",
    destinationRoot: "",
    cardReaderPath: "",
    currentOrder: 1,
  }),

  getters: {
    destinationPath(): string {
      if (!this.destinationRoot || !this.eventSlug) return "";
      const orderPadded = String(this.currentOrder).padStart(4, "0");
      return `${this.destinationRoot}/originals/${this.eventSlug}/${this.photographer}/${this.gym}/${this.sessionNumber}/${orderPadded}`;
    },

    isConfigured(): boolean {
      return Boolean(
        this.eventSlug &&
          this.photographer &&
          this.gym &&
          this.sessionNumber &&
          this.destinationRoot
      );
    },
  },

  actions: {
    incrementOrder() {
      this.currentOrder++;
    },

    reset() {
      this.eventSlug = "";
      this.photographer = "";
      this.gym = "";
      this.sessionNumber = "";
      this.destinationRoot = "";
      this.cardReaderPath = "";
      this.currentOrder = 1;
    },
  },

  persist: true,
});
